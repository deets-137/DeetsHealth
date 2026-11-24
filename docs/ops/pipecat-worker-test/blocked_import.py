"""Import the typed call's Pipecat modules with the packages Pyodide lacks made unimportable.

Native CPython, so it proves only one thing: whether the typed path needs those packages at import.
"""

import importlib
import sys

BLOCKED = ("onnxruntime", "loudness", "pipecat_resampler", "sentencex", "soundfile", "_soundfile", "grpc")


class Block:
    def find_spec(self, name, path=None, target=None):
        if name.split(".")[0] in BLOCKED:
            raise ImportError(f"blocked for the test: {name}")
        return None


sys.meta_path.insert(0, Block())

MODULES = [
    "pipecat.frames.frames",
    "pipecat.pipeline.pipeline",
    "pipecat.pipeline.worker",
    "pipecat.workers.runner",
    "pipecat.processors.frame_processor",
    "pipecat.processors.aggregators.llm_context",
    "pipecat.processors.aggregators.llm_response_universal",
    "pipecat.processors.frameworks.rtvi",
    "pipecat.processors.frameworks.rtvi.observer",
    "pipecat.transports.base_transport",
    "pipecat.services.anthropic.llm",
    "pipecat.flows",
    "deets_agent.flow",
    "deets_agent.llm",
    "deets_agent.call",
]

for name in MODULES:
    try:
        importlib.import_module(name)
        print(f"ok      {name}")
    except BaseException as e:  # noqa: BLE001 - the test reports every failure
        chain = e
        while chain.__cause__ or chain.__context__:
            chain = chain.__cause__ or chain.__context__
        print(f"FAILED  {name}: {type(chain).__name__}: {str(chain)[:160]}")
