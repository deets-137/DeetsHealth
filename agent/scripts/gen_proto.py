"""Generates the Python code for the ehr contract (EHR-SERVICE.md §3).

    uv run python scripts/gen_proto.py

Reads ../ehr/proto/deets/ehr/v1/ehr.proto, the one source, and writes deets_agent/proto/deets/
(git-ignored). grpcio-tools carries its own protoc, so nothing is installed by hand. Run it
again after any change to the contract.
"""

import shutil
import sys
from importlib import resources
from pathlib import Path

from grpc_tools import protoc

AGENT = Path(__file__).resolve().parents[1]
PROTO_ROOT = AGENT.parent / "ehr" / "proto"
OUT = AGENT / "deets_agent" / "proto"
CONTRACT = "deets/ehr/v1/ehr.proto"


def main() -> int:
    shutil.rmtree(OUT / "deets", ignore_errors=True)
    wkt = resources.files("grpc_tools") / "_proto"
    code = protoc.main(
        [
            "protoc",
            f"-I{PROTO_ROOT}",
            f"-I{wkt}",
            f"--python_out={OUT}",
            f"--pyi_out={OUT}",
            f"--grpc_python_out={OUT}",
            str(PROTO_ROOT / CONTRACT),
        ]
    )
    if code != 0:
        print("protoc failed", file=sys.stderr)
        return code
    # Make each generated folder a package.
    for folder in (OUT / "deets").rglob("*"):
        if folder.is_dir():
            (folder / "__init__.py").touch()
    (OUT / "deets" / "__init__.py").touch()
    print(f"wrote the ehr contract's Python code to {OUT / 'deets'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
