"""The PHI-free log processor (SECURITY.md §4.5; BUILD-ORDER.md Phase 1), in from day one.

Logs carry counts, Flow nodes, durations and error kinds, never a value. Two rules make that
hold without trusting every library:

1. **Our code** (`deets_agent.*`) writes only fixed text, kinds and counts. A protected value
   is a `Shielded` and prints as `***`; a wire message is printed only through
   `redacted_text`, which blanks each field the contract marks `(phi)` or `(secret)`.
2. **Every other library** (Pipecat, the Anthropic SDK, grpc) logs the context, the prompt or
   a tool's arguments at some level. Its text is withheld: the line keeps its level, module,
   function and line number, and loses its message.

Exceptions keep their type and drop their message and locals, which can hold values.
`install()` runs once, before the pipeline starts.
"""

from __future__ import annotations

import logging
import sys
from collections.abc import Callable
from contextvars import ContextVar

from google.protobuf.message import Message
from loguru import logger

from deets_agent.proto import pb

OURS = "deets_agent"
WITHHELD = "(library log text withheld: PHI wall)"


def _marked(field) -> bool:
    opts = field.GetOptions()
    return bool(opts.Extensions[pb.phi] or opts.Extensions[pb.secret])


def redacted_text(message: Message) -> str:
    """A wire message as text, with every marked field shown as `***`."""
    parts = []
    for field in message.DESCRIPTOR.fields:
        if _marked(field):
            parts.append(f"{field.name}=***")
            continue
        value = getattr(message, field.name)
        if field.message_type is not None:
            if field.is_repeated:
                inner = ", ".join(redacted_text(v) for v in value)
                parts.append(f"{field.name}=[{inner}]")
            elif message.HasField(field.name):
                parts.append(f"{field.name}={redacted_text(value)}")
        else:
            parts.append(f"{field.name}={value!r}")
    return f"{message.DESCRIPTOR.name}({', '.join(parts)})"


# The dev call record's tap (call_record.py), set by the typed harness and the dev server only:
# it gets each warning and error as written, before the wall withholds it, for a synthetic
# call's record. None in every other process. Per call, not per process (docs/ops/DEMO.md
# §3.6: several calls at once): a context variable, so the tasks a call starts inherit its own
# tap, and a warning lands in the record of the call that caused it. A warning from a thread
# that no call started (grpc's own) reaches no record.
_dev_tap: ContextVar[Callable[..., None] | None] = ContextVar("dev_tap", default=None)


def set_dev_tap(tap: Callable[..., None] | None) -> None:
    """Point this context's warnings at a call record (None: at none)."""
    _dev_tap.set(tap)


def _patch(record) -> None:
    exc = record["exception"]
    dev_tap = _dev_tap.get()
    if dev_tap is not None and record["level"].no >= 30:
        detail = f"{exc.type.__name__}: {exc.value}" if exc is not None and exc.type is not None else None
        name = record["extra"].get("lib_name") or record["name"] or ""
        text = record["extra"].get("lib_text") or record["message"]
        dev_tap(record["level"].name, f"{name}:{record['function']}:{record['line']}", text, detail)
    kind = record["extra"].get("exc_kind", "")
    if exc is not None and exc.type is not None:
        kind = exc.type.__name__
    suffix = f" [{kind}]" if kind else ""
    if "lib_name" in record["extra"]:
        record["name"] = record["extra"]["lib_name"]
    if not record["name"] or not record["name"].startswith(OURS):
        record["message"] = WITHHELD + suffix
    else:
        record["message"] = record["message"] + suffix
    record["exception"] = None
    record["extra"].clear()


_LEVELS = {"DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"}


class _StdlibToLoguru(logging.Handler):
    """Sends the standard `logging` module (grpc, the Anthropic SDK) through the same rules."""

    def emit(self, record: logging.LogRecord) -> None:
        kind = record.exc_info[0].__name__ if record.exc_info and record.exc_info[0] else ""
        level = record.levelname if record.levelname in _LEVELS else "INFO"
        extra = {"lib_name": record.name, "exc_kind": kind}
        if _dev_tap.get() is not None:
            extra["lib_text"] = record.getMessage()
        logger.bind(**extra).log(level, "")


def install(level: str = "INFO") -> None:
    """Replace every log sink with one that follows the PHI rules above."""
    logger.remove()
    logger.configure(patcher=_patch)
    logger.add(
        sys.stderr,
        level=level,
        format="{time:HH:mm:ss.SSS} {level: <7} {name}:{function}:{line} {message}",
        backtrace=False,
        diagnose=False,
    )
    logging.basicConfig(handlers=[_StdlibToLoguru()], level=logging.WARNING, force=True)
