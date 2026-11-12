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
import contextlib
import re
from datetime import date

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
    TTSSpeakFrame,
)
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineParams, PipelineWorker
from pipecat.processors.aggregators.llm_context import LLMContext
from pipecat.processors.aggregators.llm_response_universal import LLMContextAggregatorPair
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.processors.frameworks.rtvi import RTVIObserverParams
from pipecat.processors.frameworks.rtvi.observer import RTVIFunctionCallReportLevel
from pipecat.services.anthropic.llm import AnthropicLLMService
from pipecat.transports.base_transport import BaseTransport
from pipecat.workers.runner import WorkerRunner

from deets_agent import config
from deets_agent.ehr_client import EhrClient
from deets_agent.flow import CLOSING_STEPS, FAKE_TODAY, STEPS, CallFlow, role
from deets_agent.prompts import text
from deets_agent.screen import CallerScreen

# After a response ends, how long to wait for a tool call or a new response before the turn
# counts as over. Tool calls to ehr take milliseconds; this only covers frame ordering.
TURN_GRACE_S = 0.4
# The longest one bot turn may take before the harness gives up on it.
TURN_LIMIT_S = 60.0

ENDED = object()


class CallFailed(Exception):
    """Claude's request failed, so the call cannot go on. The message is the error's type only;
    the full error is in the dev call record."""


_MONTHS_DAYS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
_VALUE = re.compile(r"\d+(?::\d+)?|\b(?:" + "|".join(_MONTHS_DAYS) + r")\b", re.IGNORECASE)


def invented_values(said: str, sources: list[str]) -> list[str]:
    """The numbers, times, months and days in `said` that appear in none of `sources`."""
    known = {m.group(0).lower() for s in sources for m in _VALUE.finditer(s)}
    found = [m.group(0) for m in _VALUE.finditer(said)]
    return sorted({v for v in found if v.lower() not in known})


class TurnCollector(FrameProcessor):
    """Collects the bot's words for one turn and reports when the turn is over."""

    def __init__(self) -> None:
        super().__init__()
        self.turns: asyncio.Queue = asyncio.Queue()
        self._words: list[str] = []
        self._last_whole = False
        self._pending_calls = 0
        self._in_response = False
        self._settle: asyncio.Task | None = None
        # The invented-value check (2026-11-11): the model's own words in the current response,
        # every fixed line spoken so far, and each value the model said that no one gave it.
        self._model_text: list[str] = []
        self._fixed_lines: list[str] = []
        self.invented: list[tuple[str, list[str]]] = []
        # The call's Flow (for the role prompt and the caller's lines), set by TypedCall.
        self.flow: CallFlow | None = None
        # The dev call record and the Flow manager, set by TypedCall when a record is open.
        self.record = None
        self.manager: FlowManager | None = None

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if direction is FrameDirection.DOWNSTREAM:
            if isinstance(frame, LLMFullResponseStartFrame):
                self._in_response = True
                self._model_text.clear()
                self._cancel_settle()
                if self.record is not None and self.manager is not None:
                    self.record.node(self.manager.current_node)
            elif isinstance(frame, LLMTextFrame):
                self._add(frame.text)
                self._model_text.append(frame.text)
            elif isinstance(frame, TTSSpeakFrame):
                # A fixed line the Flow speaks itself (flow.py `_say`). If the model goes on, its
                # response starts within the grace and joins this turn; if not, the turn ends.
                self._add(frame.text, whole=True)
                self._fixed_lines.append(frame.text)
                self._cancel_settle()
                self._settle = asyncio.create_task(self._settle_turn())
            elif isinstance(frame, FunctionCallsStartedFrame):
                self._pending_calls += len(frame.function_calls)
                self._cancel_settle()
                if self.record is not None:
                    for fc in frame.function_calls:
                        self.record.tool_call(fc.tool_call_id, fc.function_name, dict(fc.arguments))
            elif isinstance(frame, FunctionCallResultFrame):
                self._pending_calls = max(0, self._pending_calls - 1)
                if self.record is not None:
                    self.record.tool_result(
                        frame.tool_call_id, frame.function_name, frame.result, getattr(frame, "error", None)
                    )
            elif isinstance(frame, LLMFullResponseEndFrame):
                self._in_response = False
                self._check_values()
                self._cancel_settle()
                self._settle = asyncio.create_task(self._settle_turn())
            elif isinstance(frame, EndFrame):
                self._cancel_settle()
                self._flush()
                await self.turns.put(ENDED)
        await self.push_frame(frame, direction)

    def _check_values(self) -> None:
        """Flag each value in the model's own words that no one gave it: a number, a month or a
        day that is not in the role prompt, a caller's line or a fixed line. It finds an invented
        date or time; it does not stop the caller hearing it (AGENT.md §3)."""
        said = "".join(self._model_text).strip()
        self._model_text.clear()
        if not said or self.flow is None:
            return
        values = invented_values(
            said, [role(self.flow.today), *self._fixed_lines, *(x.reveal() for x in self.flow.caller_lines)]
        )
        if values:
            self.invented.append((said, values))
            if self.record is not None:
                self.record.event("invented_value", values=values, text=said)

    def _add(self, text: str, whole: bool = False) -> None:
        """Add words to the turn. A whole line (spoken by the Flow) is set apart by a space from
        the model's stream on either side of it."""
        if not self._words and self.record is not None:
            # The answer's first frame: where a voice caller would start to hear it. The turn
            # itself is stamped later, after TURN_GRACE_S, which no voice caller waits through.
            self.record.answer_started()
        if (whole or self._last_whole) and self._words and not self._words[-1].endswith((" ", "\n")):
            self._words.append(" ")
        self._words.append(text)
        self._last_whole = whole

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
        self._last_whole = False
        if text:
            self.turns.put_nowait(text)


class TypedCall:
    """A call driven by text. `start()` returns the greeting; `say()` returns each reply, or
    None once the call has ended."""

    def __init__(
        self,
        ehr: EhrClient,
        llm: AnthropicLLMService,
        record=None,
        today: date = FAKE_TODAY,
        transport: BaseTransport | None = None,
    ) -> None:
        """`transport`: the caller's seat in the console (dev_server.py). The caller's lines then
        come over RTVI, and `serve()` runs the call in place of `start()` and `say()`."""
        # A typed call pins "today" to the day ehr's fake slots are built around (flow.py).
        self.flow = CallFlow(ehr, today=today)
        self._llm = llm
        self._collector = TurnCollector()
        self._collector.flow = self.flow
        # Reads each caller line before the model does (screen.py).
        self._screen = CallerScreen(self.flow)
        # The dev call record (call_record.py): the typed harness passes one; tests do not.
        self.record = record
        context = LLMContext()
        self._aggregators = LLMContextAggregatorPair(context)
        middle = [self._screen, self._aggregators.user(), llm, self._collector, self._aggregators.assistant()]
        if transport is None:
            self._worker = PipelineWorker(
                Pipeline(middle), params=PipelineParams(), cancel_on_idle_timeout=False, enable_rtvi=False
            )
        else:
            # The caller's seat (CHAT.md §2.4): RTVI carries the caller's lines in and the call's
            # state out. Tool reports stay at NONE, so the browser never gets a tool's arguments
            # or result through RTVI; the watch stream carries those, from the record.
            self._worker = PipelineWorker(
                Pipeline([transport.input(), *middle, transport.output()]),
                params=PipelineParams(enable_metrics=True, enable_usage_metrics=True),
                enable_rtvi=True,
                rtvi_observer_params=RTVIObserverParams(
                    function_call_report_level={"*": RTVIFunctionCallReportLevel.NONE}
                ),
            )
        global_functions = [
            self.flow.hand_off_fn(),
            self.flow.end_call_fn(),
            self.flow.emergency_fn(),
            self.flow.safety_reminder_fn(),
            self.flow.medical_question_fn(),
            self.flow.flag_for_staff_fn(),
        ]
        self._manager = FlowManager(
            llm=llm,
            context_aggregator=self._aggregators,
            worker=self._worker,
            global_functions=global_functions,
        )
        self._runner = WorkerRunner()
        self._run_task: asyncio.Task | None = None
        self.ended = False
        # Set when a turn failed (a failed request, or no reply within the limit): hang_up()
        # then cancels the pipeline in place of waiting for it to end.
        self.failed = False
        client = getattr(llm, "_client", None)
        messages = getattr(client, "messages", None)
        if messages is not None and hasattr(messages, "on_failure"):
            # A failed request kills Pipecat's LLM service; end the turn now, not after the limit.
            messages.on_failure = lambda e: self._collector.turns.put_nowait(CallFailed(type(e).__name__))
        # Review flags (FLAGS.md §8): marks for an analyst or engineer, never for a nurse.
        self.review_flags: list[str] = []
        if messages is not None and hasattr(messages, "on_cut_off"):

            async def on_cut_off(final: bool) -> None:
                # A reply cut off by the token cap (llm.py `_CutOffGuard`; the owner, 2026-11-12):
                # the caller hears the holding line at once, pushed out of the LLM service itself
                # (a frame queued at the pipeline's start would wait behind the reply), and code
                # marks the call for review.
                self.review_flags.append("token_cap")
                if record is not None:
                    record.event("review_flag", reason="token_cap", final=final)
                if not final:
                    await llm.push_frame(TTSSpeakFrame(text=text("line.one_moment"), append_to_context=False))

            messages.on_cut_off = on_cut_off
        if record is not None:
            self._collector.record = record
            self._collector.manager = self._manager
            self._screen.record = record
            ehr.record = record
            # Each staff flag goes to the record, and so to the watch stream (FLAGS.md §5).
            self.flow.on_flag = lambda flag: record.event("staff_flag", **flag.test_record())
            # Each move between steps, when it happens, with the tools the model has there
            # (CHAT.md §11).
            everywhere = [f.name for f in global_functions]
            self.flow.on_step = lambda name, tools: record.event(
                "step",
                name=name,
                tools=tools,
                global_tools=everywhere,
                steps=list(STEPS),
                closing_steps=list(CLOSING_STEPS),
            )
            if messages is not None and hasattr(messages, "record"):
                messages.record = record
        self._found: list[tuple[str, str]] = getattr(messages, "words_before_tool", [])

    @property
    def words_before_silent_tool(self) -> list[tuple[str, str]]:
        """The model's words that came before end_call, hand_off or emergency in one reply (llm.py)."""
        return [(n, t) for n, t in self._found if n in config.SILENT_TOOLS]

    @property
    def invented(self) -> list[tuple[str, list[str]]]:
        """Each reply of the model's that held a value no one gave it (TurnCollector)."""
        return self._collector.invented

    @property
    def screen_fired(self) -> bool:
        """True once the caller screen spoke the 911 line on this call (screen.py)."""
        return self._screen.fired

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

    async def serve(self) -> str:
        """Run a call whose caller is on a transport, until it ends. Each of the agent's turns
        goes to the record (and so to the watch stream). Returns the end's reason."""
        if self.record is not None:
            self._screen.on_line = self.record.caller
        started = asyncio.Event()

        @self._worker.event_handler("on_pipeline_started")
        async def _on_started(worker, frame):
            started.set()

        await self.flow.start()
        await self._runner.add_workers(self._worker)
        self._run_task = asyncio.create_task(self._runner.run())
        await asyncio.wait_for(started.wait(), timeout=10)
        await self._manager.initialize(self.flow.greet())
        reason = "the caller hung up"
        try:
            while True:
                getter = asyncio.create_task(self._collector.turns.get())
                done, _ = await asyncio.wait({getter, self._run_task}, return_when=asyncio.FIRST_COMPLETED)
                if getter not in done:
                    getter.cancel()
                    break  # the pipeline stopped: the caller's socket closed
                turn = getter.result()
                if isinstance(turn, CallFailed):
                    reason = f"Claude's request failed ({turn})"
                    break
                if turn is ENDED:
                    reason = "the agent ended it"
                    break
                if self.record is not None:
                    self.record.agent(turn)
        finally:
            self.ended = True
            if not self._run_task.done():
                await self._worker.queue_frame(EndFrame())
                try:
                    await asyncio.wait_for(self._run_task, timeout=10)
                except TimeoutError:
                    self._run_task.cancel()
            await self.flow.close()
        return reason

    async def cancel(self) -> None:
        """Stop a served call at once: the caller's socket closed."""
        await self._worker.cancel(reason="the caller hung up")

    async def say(self, text: str) -> str | None:
        if self.ended:
            return None
        if self.record is not None:
            self.record.caller(text)
        await self._worker.queue_frame(
            LLMMessagesAppendFrame(messages=[{"role": "user", "content": text}], run_llm=True)
        )
        return await self._next_turn()

    async def _next_turn(self) -> str | None:
        try:
            turn = await asyncio.wait_for(self._collector.turns.get(), timeout=TURN_LIMIT_S)
        except TimeoutError:
            self.failed = True
            raise
        if isinstance(turn, CallFailed):
            self.failed = True
            raise turn
        if self.record is not None and turn is not ENDED:
            self.record.agent(turn)
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
        """End the call; safe after a failed request and safe to call twice. A failed request
        kills Pipecat's LLM service, so an EndFrame may never get through: the pipeline is then
        cancelled. Before 2026-11-12 a test whose reply was not recorded left the pipeline
        running, and pytest hung at the end of the run in place of failing."""
        if not self.ended and not self.failed:
            await self._worker.queue_frame(EndFrame())
        self.ended = True
        task = self._run_task
        if task is not None and not task.done():
            if self.failed:
                await self._worker.cancel(reason="the call failed")
            try:
                await asyncio.wait_for(asyncio.shield(task), timeout=10)
            except TimeoutError:
                task.cancel()
                with contextlib.suppress(BaseException):  # only stopping it matters here
                    await task
        await self.flow.close()
