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
    if kind == "screen":
        return f"{t}    !!    SCREEN {e['action']}"
    if kind == "invented_value":
        return f"{t}    !!    INVENTED VALUE {', '.join(e['values'])}: {_short(e['text'])}"
    if kind == "words_before_tool":
        # On a voice call these words are spoken before the tool runs (AGENT.md §3).
        flag = "WORDS BEFORE A FIXED-LINE TOOL" if e.get("silent", e.get("closing")) else "words before tool"
        return f"{t}    !!    {flag} {e['name']}: {_short(e['text'])}"
    if kind == "ehr":
        return f"{t}    ehr   {e['call']} {e['ms']} ms {e['outcome']}" if show_all else None
    if kind == "epic":
        # ehr's own request to the EHR (the ledger, from CloseCall); `at` is from the call's start.
        at = f"{e['at_ms'] / 1000:7.2f}s"
        return (
            f"{at}    epic  [{e['rpc']}] {e['what']} · {e['status']} · {e['results']} results · "
            f"{e['bytes']} B · {e['ms']} ms"
        )
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
        # The first word is what a caller waits for on a voice call; the first event can be thinking.
        first += f" words {e['first_text_ms']} ms," if "first_text_ms" in e else ""
        first += f" tool {e['first_tool_ms']} ms," if "first_tool_ms" in e else ""
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
            f"{r['replayed']} replayed, {r['failed']} failed"
            + (f" - about ${e['cost_usd']:.4f}" if isinstance(e.get("cost_usd"), (int, float)) else "")
        )
    return f"{t}  {kind} {_short(e)}"


def _sent(e: dict) -> list[str]:
    """What one request sent to Claude: the system prompt when it changed, then each message
    added since the last request (or all of them, when the context was rewritten)."""
    out: list[str] = []
    if "system" in e:
        out.append(f"system (changed): {_short(json.dumps(e['system'], ensure_ascii=False), 300)}")
    messages = e.get("new_messages", e.get("all_messages", []))
    if "all_messages" in e:
        out.append(f"(all {len(messages)} messages, the context was rewritten)")
    for m in messages:
        content = m.get("content")
        if isinstance(content, list):
            parts = []
            for b in content:
                kind = b.get("type")
                if kind == "text":
                    parts.append(b.get("text", ""))
                elif kind == "tool_use":
                    parts.append(f"<tool_use {b.get('name')} {json.dumps(b.get('input'), ensure_ascii=False)}>")
                elif kind == "tool_result":
                    parts.append(f"<tool_result {b.get('content')}>")
                else:
                    parts.append(f"<{kind}>")
            content = " ".join(parts)
        out.append(f"{m.get('role')}: {_short(str(content), 300)}")
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("file", nargs="?", type=Path)
    parser.add_argument("--all", action="store_true", help="show ehr calls and replayed requests too")
    parser.add_argument(
        "--requests", action="store_true", help="show what was sent to Claude on each request (schema 12)"
    )
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
        e = json.loads(raw)
        text = line(e, args.all or args.requests)
        if text is not None:
            print(text)
        if args.requests and e.get("kind") == "llm":
            for sent in _sent(e):
                print(f"            {sent}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
