"""RTVI as plain JSON text over the caller's WebSocket (CHAT.md §5).

Pipecat's own browser WebSocket transport speaks protobuf and pulls in `daily-js`, which loads
Daily's call-machine bundle from Daily's CDN: remote code in a page that will show protected
data (SECURITY.md §4). So the console uses `@pipecat-ai/client-js` with its own small text
transport (console/src/agent/transport.ts), and this serializer is the server's half: one RTVI
message per WebSocket text frame, no audio.

It never logs a message: a caller's line is protected data.
"""

from __future__ import annotations

import json

from pipecat.frames.frames import (
    Frame,
    InputTransportMessageFrame,
    OutputTransportMessageFrame,
    OutputTransportMessageUrgentFrame,
)
from pipecat.serializers.base_serializer import FrameSerializer

# The longest message the caller's seat may send: a typed line plus RTVI's envelope.
MAX_MESSAGE = 8_000


class RtviJsonSerializer(FrameSerializer):
    def __init__(self) -> None:
        # Pipecat's serializers drop RTVI messages by default (they are for telephony wires).
        # On this wire RTVI is the whole point.
        super().__init__(FrameSerializer.InputParams(ignore_rtvi_messages=False))

    async def serialize(self, frame: Frame) -> str | bytes | None:
        if isinstance(frame, (OutputTransportMessageFrame, OutputTransportMessageUrgentFrame)):
            if self.should_ignore_frame(frame):
                return None
            return json.dumps(frame.message, ensure_ascii=False, default=str)
        return None  # no audio, no other frames: the text seat has none

    async def deserialize(self, data: str | bytes) -> Frame | None:
        if isinstance(data, bytes) or len(data) > MAX_MESSAGE:
            return None
        try:
            message = json.loads(data)
        except json.JSONDecodeError:
            return None
        if not isinstance(message, dict) or message.get("label") != "rtvi-ai":
            return None
        return InputTransportMessageFrame(message=message)
