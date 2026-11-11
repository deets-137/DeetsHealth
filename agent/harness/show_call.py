"""Print a dev call record (call_record.py) as a readable timeline.

    uv run python harness/show_call.py            # the newest record in agent/calls/
    uv run python harness/show_call.py FILE       # one record
    uv run python harness/show_call.py --all      # every event, also replayed Claude requests

Synthetic calls only: a record holds both sides of the conversation.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from deets_agent.call_record import CALLS


def _short(value, limit: int = 160) -> str:
    text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
    return text if len(text) <= limit else text[: limit - 3] + "..."


def line(e: dict, show_all: bool) -> str | None:
    t = f"{e['t_ms'] / 1000:7.2f}s"
    kind = e["kind"]
    if kind == "call_start":
        return f"{t}  CALL START  mode={e['mode']} model={e['model']} at {e['started']}"
    if kind == "caller":
        return f"{t}  you    > {e['text']}"
    if kind == "agent":
        return f"{t}  agent  > {e['text']}"
    if kind == "node":
        return f"{t}  -- node: {e['name']}"
    if kind == "tool_call":
        return f"{t}    tool  {e['name']}({_short(e['arguments'])})"
    if kind == "tool_result":
        ms = f" {e['ms']} ms" if "ms" in e else ""
        err = f"  ERROR {e['error']}" if e.get("error") else ""
        return f"{t}    <-    {e['name']}{ms}: {_short(e['result'])}{err}"
    if kind == "ehr":
        return f"{t}    ehr   {e['call']} {e['ms']} ms {e['outcome']}" if show_all else None
    if kind == "llm":
        if "error" in e:
            err = e["error"]
            status = f" {err['status_code']}" if "status_code" in err else ""
            return f"{t}    LLM FAILED [{e['key']}] {err['type']}{status}: {err['message']}"
        if e.get("hit") and not show_all:
            return None
        u = e.get("usage", {})
        src = "replay" if e.get("hit") else "live"
        first = f" first {e['first_event_ms']} ms," if "first_event_ms" in e else ""
        dropped = f" dropped {e['thinking_dropped']} thinking," if e.get("thinking_dropped") else ""
        return (
            f"{t}    llm   [{e['key']}] {src},{first} total {e['total_ms']} ms,{dropped} "
            f"in {u.get('input_tokens', 0)} / cache read {u.get('cache_read_input_tokens', 0)} / "
            f"cache write {u.get('cache_creation_input_tokens', 0)} / out {u.get('output_tokens', 0)}"
        )
    if kind == "log":
        exc = f"\n            {e['exception']}" if e.get("exception") else ""
        return f"{t}  {e['level']} {e['where']}: {e['text']}{exc}"
    if kind == "error":
        return f"{t}  HARNESS ERROR {e['type']}: {e['message']}"
    if kind == "call_end":
        r = e["requests"]
        return (
            f"{t}  CALL END  {e['reason']} - {e['turns']} caller turns - Claude {r['live']} live, "
            f"{r['replayed']} replayed, {r['failed']} failed - about ${e['cost_usd']:.4f}"
        )
    return f"{t}  {kind} {_short(e)}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("file", nargs="?", type=Path)
    parser.add_argument("--all", action="store_true", help="show ehr calls and replayed requests too")
    args = parser.parse_args()
    # The Windows console is not UTF-8 by default; a caller's words may not fit its code page.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    path = args.file
    if path is None:
        records = sorted(CALLS.glob("*.jsonl"))
        if not records:
            print(f"no records in {CALLS}")
            return 1
        path = records[-1]
    print(path)
    for raw in path.read_text(encoding="utf-8").splitlines():
        text = line(json.loads(raw), args.all)
        if text is not None:
            print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
