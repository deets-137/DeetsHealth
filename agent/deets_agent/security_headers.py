"""The console's response headers (CONSOLE.md §5.1), read from `console/security-headers.json`,
the one source the deploy reads too. The dev server sends the page's on the demo's files and the
API's on every other reply; Vite sends the page's on this PC (console/vite.config.ts).

Also the violation reports (`POST /agent/csp-report`): a log line names the directive and what
was blocked as a kind (`inline`, `eval`, `trusted-types-sink`) or a scheme and host. Never the
page's path, a query or a sample (the policy has no `'report-sample'`).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from deets_agent import config

FILE = config.REPO / "console" / "security-headers.json"

# The largest report body taken: a browser's is under a kilobyte.
MAX_REPORT_BYTES = 8 * 1024
# The kinds a browser puts in `blocked-uri` in place of an address.
KINDS = {"inline", "eval", "wasm-eval", "trusted-types-sink", "trusted-types-policy", "self", "data", "blob"}
_DIRECTIVE = re.compile(r"[a-z-]{1,40}")
_HOST = re.compile(r"[a-z0-9.-]{1,253}(:[0-9]{1,5})?")


class SecurityHeaders:
    """The headers, read once at start. A missing or broken file stops the start."""

    def __init__(self, spec: dict[str, Any]) -> None:
        self.spec = spec
        # Each (directive, blocked) pair seen, with its count: the first is printed, then every
        # hundredth, so a page in a loop cannot flood the terminal.
        self.seen: dict[tuple[str, str], int] = {}

    @classmethod
    def load(cls, file: Path = FILE) -> SecurityHeaders:
        return cls(json.loads(file.read_text(encoding="utf-8")))

    def policy(self, base: str, dev: bool = False) -> str:
        """The page's Content-Security-Policy, with its report address under `base` (`/` on
        this PC, `/demo/` in the demo)."""
        directives = dict(self.spec["csp"])
        if dev:
            directives.update(self.spec["dev"])
        parts = [f"{name} {' '.join(values)}" for name, values in directives.items() if values is not None]
        parts.append(f"report-uri {base}{self.spec['report']}")
        return "; ".join(parts)

    def page(self, base: str, demo: bool) -> dict[str, str]:
        """The headers on the console's own files."""
        headers = {"Content-Security-Policy": self.policy(base), **self.spec["page"]}
        if demo:
            headers.update(self.spec["demo"])
        return headers

    def api(self) -> dict[str, str]:
        """The headers on every reply that is not one of the console's files."""
        return dict(self.spec["api"])

    def report(self, body: bytes) -> tuple[str, str] | None:
        """One violation report: the directive and what was blocked, each checked to a safe
        shape, and printed when it is new (or every hundredth time). None when it is not a
        report."""
        try:
            data = json.loads(body)
        except (ValueError, UnicodeDecodeError):
            return None
        report = data.get("csp-report") if isinstance(data, dict) else None
        if not isinstance(report, dict):
            return None
        directive = str(report.get("effective-directive") or report.get("violated-directive") or "")
        directive = directive.split(" ", 1)[0]
        if not _DIRECTIVE.fullmatch(directive):
            directive = "?"
        pair = (directive, _blocked(report.get("blocked-uri")))
        count = self.seen.get(pair, 0) + 1
        self.seen[pair] = count
        if count == 1 or count % 100 == 0:
            times = "" if count == 1 else f" ({count} times)"
            print(f"csp: blocked by {pair[0]}: {pair[1]}{times}", file=sys.stderr, flush=True)
        return pair


def _blocked(value: Any) -> str:
    """A kind, or a scheme and host. Never a path or a query."""
    text = str(value or "").strip().lower()
    if text in KINDS:
        return text
    if text.endswith(":") and text[:-1] in KINDS:
        return text[:-1]
    parts = urlsplit(text)
    if parts.scheme in ("http", "https", "ws", "wss") and _HOST.fullmatch(parts.netloc or ""):
        return f"{parts.scheme}://{parts.netloc}"
    if parts.scheme in KINDS:
        return parts.scheme
    return "?"
