"""The code generated from the ehr contract (scripts/gen_proto.py; never edit it).

The generated modules import each other as `deets.ehr.v1`, so this folder goes on the path.
"""

import sys
from pathlib import Path

_HERE = str(Path(__file__).parent)
if _HERE not in sys.path:
    sys.path.insert(0, _HERE)

try:
    from deets.ehr.v1 import ehr_pb2 as pb  # noqa: E402
    from deets.ehr.v1 import ehr_pb2_grpc as pb_grpc  # noqa: E402
except ModuleNotFoundError as e:  # pragma: no cover - a setup step, not a code path
    raise ModuleNotFoundError(
        "The ehr contract's Python code is missing. Run: uv run python scripts/gen_proto.py"
    ) from e

__all__ = ["pb", "pb_grpc"]
