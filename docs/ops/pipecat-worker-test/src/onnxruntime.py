"""Stand-in for `onnxruntime` (no Pyodide build). Pipecat's default turn strategy builds a
smart-turn model session even for a typed call; it only runs on audio. A session can be made;
running one fails."""

import enum


class ExecutionMode(enum.Enum):
    ORT_SEQUENTIAL = 0
    ORT_PARALLEL = 1


class GraphOptimizationLevel(enum.Enum):
    ORT_DISABLE_ALL = 0
    ORT_ENABLE_ALL = 99


class SessionOptions:
    pass


class InferenceSession:
    def __init__(self, *args, **kwargs) -> None:
        pass

    def run(self, *args, **kwargs):
        raise RuntimeError("onnxruntime: no audio models in this Worker")
