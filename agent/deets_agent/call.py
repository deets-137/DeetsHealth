"""One call with text in and text out: the real Flow, the real LLM service, the real ehr, no
audio (test layer 3, VOICE-AGENT.md §9). Phase 3 swaps the text ends for Cartesia and Twilio;
the middle stays.

    user text ─► user aggregator ─► Claude ─► TurnCollector ─► assistant aggregator

A typed turn enters the way Pipecat's own send-text does: an `LLMMessagesAppendFrame` with
`run_llm=True`. The bot's turn is over when a response ends with no tool call still running
and no new response starts within a short grace.
"""

from __future__ import annotations

import asyncio

from pipecat.flows import FlowManager
from pipecat.frames.frames import (
    EndFrame,
    Frame,
    FunctionCallResultFrame,
    FunctionCallsStartedFrame,
    LLMFullResponseEndFrame,
    LLMFullResponseStartFrame,
    LLMMessagesAppendFrame,
    LLMTextFrame,
)
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineParams, PipelineWorker
from pipecat.processors.aggregators.llm_context import LLMContext
from pipecat.processors.aggregators.llm_response_universal import LLMContextAggregatorPair
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.services.anthropic.llm import AnthropicLLMService
from pipecat.workers.runner import WorkerRunner

from deets_agent.ehr_client import EhrClient
from deets_agent.flow import CallFlow

# After a response ends, how long to wait for a tool call or a new response before the turn
# counts as over. Tool calls to ehr take milliseconds; this only covers frame ordering.
TURN_GRACE_S = 0.4
# The longest one bot turn may take before the harness gives up on it.
TURN_LIMIT_S = 60.0

ENDED = object()


class TurnCollector(FrameProcessor):
    """Collects the bot's words for one turn and reports when the turn is over."""

    def __init__(self) -> None:
        super().__init__()
        self.turns: asyncio.Queue = asyncio.Queue()
        self._words: list[str] = []
        self._pending_calls = 0
        self._in_response = False
        self._settle: asyncio.Task | None = None

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if direction is FrameDirection.DOWNSTREAM:
            if isinstance(frame, LLMFullResponseStartFrame):
                self._in_response = True
                self._cancel_settle()
            elif isinstance(frame, LLMTextFrame):
                self._words.append(frame.text)
            elif isinstance(frame, FunctionCallsStartedFrame):
                self._pending_calls += len(frame.function_calls)
                self._cancel_settle()
            elif isinstance(frame, FunctionCallResultFrame):
                self._pending_calls = max(0, self._pending_calls - 1)
            elif isinstance(frame, LLMFullResponseEndFrame):
                self._in_response = False
                self._cancel_settle()
                self._settle = asyncio.create_task(self._settle_turn())
            elif isinstance(frame, EndFrame):
                self._cancel_settle()
                self._flush()
                await self.turns.put(ENDED)
        await self.push_frame(frame, direction)

    def _cancel_settle(self) -> None:
        if self._settle is not None:
            self._settle.cancel()
            self._settle = None

    async def _settle_turn(self) -> None:
        await asyncio.sleep(TURN_GRACE_S)
        if self._pending_calls == 0 and not self._in_response:
            self._flush()

    def _flush(self) -> None:
        text = "".join(self._words).strip()
        self._words.clear()
        if text:
            self.turns.put_nowait(text)


class TypedCall:
    """A call driven by text. `start()` returns the greeting; `say()` returns each reply, or
    None once the call has ended."""

    def __init__(self, ehr: EhrClient, llm: AnthropicLLMService) -> None:
        self.flow = CallFlow(ehr)
        self._llm = llm
        self._collector = TurnCollector()
        context = LLMContext()
        self._aggregators = LLMContextAggregatorPair(context)
        pipeline = Pipeline(
            [self._aggregators.user(), llm, self._collector, self._aggregators.assistant()]
        )
        self._worker = PipelineWorker(
            pipeline, params=PipelineParams(), cancel_on_idle_timeout=False, enable_rtvi=False
        )
        self._manager = FlowManager(
            llm=llm,
            context_aggregator=self._aggregators,
            worker=self._worker,
            global_functions=[self.flow.hand_off_fn()],
        )
        self._runner = WorkerRunner()
        self._run_task: asyncio.Task | None = None
        self.ended = False

    async def start(self) -> str | None:
        started = asyncio.Event()

        @self._worker.event_handler("on_pipeline_started")
        async def _on_started(worker, frame):
            started.set()

        await self.flow.start()
        await self._runner.add_workers(self._worker)
        self._run_task = asyncio.create_task(self._runner.run())
        await asyncio.wait_for(started.wait(), timeout=10)
        await self._manager.initialize(self.flow.greet())
        return await self._next_turn()

    async def say(self, text: str) -> str | None:
        if self.ended:
            return None
        await self._worker.queue_frame(
            LLMMessagesAppendFrame(messages=[{"role": "user", "content": text}], run_llm=True)
        )
        return await self._next_turn()

    async def _next_turn(self) -> str | None:
        turn = await asyncio.wait_for(self._collector.turns.get(), timeout=TURN_LIMIT_S)
        if turn is ENDED:
            await self._ended()
            return None
        if self.flow.closing:
            # The last words of the call; the end follows them.
            try:
                if await asyncio.wait_for(self._collector.turns.get(), timeout=10) is ENDED:
                    await self._ended()
            except TimeoutError:
                pass
        return turn

    async def _ended(self) -> None:
        self.ended = True
        await self.flow.close()

    async def hang_up(self) -> None:
        if not self.ended:
            await self._worker.queue_frame(EndFrame())
            self.ended = True
        if self._run_task is not None:
            await asyncio.wait_for(self._run_task, timeout=10)
        await self.flow.close()
