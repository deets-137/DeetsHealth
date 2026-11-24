"""Stand-in for `grpc.aio`: a channel that plays a tape of recorded ehr replies."""

import base64
from collections import defaultdict, deque


class AioRpcError(Exception):
    def __init__(self, code) -> None:
        super().__init__(code.name)
        self._code = code

    def code(self):
        return self._code


class Channel:
    pass


class TapeChannel(Channel):
    """`tape`: [{"name": "Book", "reply": base64}] or {"name", "error": "NOT_FOUND"}, in order."""

    def __init__(self, tape: list[dict]) -> None:
        self._left: dict[str, deque] = defaultdict(deque)
        for entry in tape:
            self._left[entry["name"]].append(entry)
        self.played: list[str] = []

    def unary_unary(self, path, request_serializer=None, response_deserializer=None, **_):
        name = path.rsplit("/", 1)[-1]

        async def call(request, timeout=None):
            import grpc

            request_serializer(request)  # the request must still serialize, as on the wire
            if not self._left[name]:
                raise AioRpcError(grpc.StatusCode.UNAVAILABLE)
            entry = self._left[name].popleft()
            self.played.append(name)
            if "error" in entry:
                raise AioRpcError(grpc.StatusCode[entry["error"]])
            return response_deserializer(base64.b64decode(entry["reply"]))

        return call

    def __getattr__(self, name):
        if name in ("unary_stream", "stream_unary", "stream_stream"):
            return lambda *a, **k: None
        raise AttributeError(name)

    async def close(self) -> None:
        return None
