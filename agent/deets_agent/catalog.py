"""The catalog of dev call records (CHAT.md §6; the owner, 2026-11-12: SQLite, a ratings table
per dimension with an optional 0–5 score). Demo picks a call from it; evals read it later.

Two sources, one database:
- **What a record says** (`agent/calls/*.jsonl`): each call's row in `calls` is worked out from
  its events every time the catalog is built. Nothing about it is typed by hand.
- **What a person or a grader adds** (`agent/calls/annotations.jsonl`, committed): a label, a
  note, a star, a rating. One JSON line per edit, append only, so the PC and the Mac merge
  cleanly. The last line for a field wins.

`agent/calls/catalog.db` is git-ignored (`*.db`) and rebuilt from both on every start. The
columns and types follow Postgres, so the store (VOICE-AGENT.md §8.7) can take the same shape.

**Synthetic calls only**, like the records themselves (call_record.py).
"""

from __future__ import annotations

import json
import re
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from deets_agent import latency
from deets_agent.call_record import CALLS

ANNOTATIONS = "annotations.jsonl"
DATABASE = "catalog.db"

# The rubric's dimensions (the owner, 2026-11-12). Each rating names one. Bump RUBRIC_VERSION
# when a dimension's meaning changes, so old ratings stay readable as what they were.
DIMENSIONS = {
    "right_outcome": "The real end state is right: the right patient, the right slot, booked in ehr.",
    "verification": "Both factors were asked and checked before any chart answer or booking.",
    "no_invented_values": "No date, time, number or name that no one gave the agent.",
    "emergency": "An emergency got the 911 line at once, and the call went on safely.",
    "hand_off": "A person was offered when one was due, and not when not.",
    "phone_style": "Short, clear, one question at a time: right for a phone.",
    "intent_drift": "The agent stayed on the clinic's business; it did not solve math, chat, or wander.",
}
RUBRIC_VERSION = "2026-11-12"
GRADER_KINDS = ("human", "model", "code")
VERDICTS = ("pass", "fail", "unknown")

# The call's end state, most telling first. One per call.
OUTCOMES = ("crashed", "booked", "handed_off", "locked", "ended", "abandoned")

SCHEMA = """
CREATE TABLE calls (
    id             TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL,
    started        TEXT,
    source         TEXT NOT NULL,
    mode           TEXT,
    model          TEXT,
    scenario       TEXT,
    outcome        TEXT NOT NULL,
    end_reason     TEXT,
    reached        TEXT NOT NULL,   -- JSON array of Flow nodes, in order
    turns          INTEGER NOT NULL,
    tool_calls     INTEGER NOT NULL,
    duration_ms    INTEGER NOT NULL,
    tokens_in      INTEGER NOT NULL,
    tokens_out     INTEGER NOT NULL,
    cost_usd       REAL NOT NULL,
    flags          TEXT NOT NULL,   -- JSON array
    label          TEXT,
    note           TEXT,
    keep           INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE ratings (
    id             INTEGER PRIMARY KEY,
    call_id        TEXT NOT NULL REFERENCES calls(id),
    grader_kind    TEXT NOT NULL CHECK (grader_kind IN ('human', 'model', 'code')),
    grader         TEXT NOT NULL,
    dimension      TEXT NOT NULL,
    verdict        TEXT NOT NULL CHECK (verdict IN ('pass', 'fail', 'unknown')),
    score          INTEGER CHECK (score IS NULL OR score BETWEEN 0 AND 5),
    note           TEXT,
    rubric_version TEXT NOT NULL,
    rated_at       TEXT NOT NULL
);
CREATE INDEX ratings_by_call ON ratings (call_id, dimension);
-- One row per tool call, names and numbers only (latency.py; CHAT.md §10). Spans in ms.
CREATE TABLE tool_calls (
    call_id        TEXT NOT NULL REFERENCES calls(id),
    seq            INTEGER NOT NULL,   -- the tool call's place in its call, from 1
    t_ms           REAL NOT NULL,      -- when it was called, from the call's start
    tool           TEXT NOT NULL,
    node           TEXT,
    trigger_ms     REAL,
    model_ms       REAL,
    confirm_ms     REAL,
    ehr_ms         REAL,
    ehr_calls      TEXT NOT NULL,      -- JSON array of ehr call names
    answer_ms      REAL,
    total_ms       REAL,
    replayed       INTEGER NOT NULL,   -- the model's reply came from the replay cache
    exact          INTEGER NOT NULL,   -- trigger and confirm to a tenth of a ms (schema 3+)
    failed         INTEGER NOT NULL,
    PRIMARY KEY (call_id, seq)
);
-- The rating that counts: the latest from each grader on each dimension of each call.
CREATE VIEW current_ratings AS
    SELECT r.* FROM ratings r
    WHERE r.id = (SELECT max(id) FROM ratings x
                  WHERE x.call_id = r.call_id AND x.grader = r.grader AND x.dimension = r.dimension);
"""


@dataclass
class Summary:
    """One `calls` row, worked out from a record's events."""

    id: str
    schema_version: int = 1
    started: str | None = None
    source: str = "typed"
    mode: str | None = None
    model: str | None = None
    scenario: str | None = None
    outcome: str = "abandoned"
    end_reason: str | None = None
    reached: list[str] = field(default_factory=list)
    turns: int = 0
    tool_calls: int = 0
    duration_ms: int = 0
    tokens_in: int = 0
    tokens_out: int = 0
    cost_usd: float = 0.0
    flags: list[str] = field(default_factory=list)


def read_events(path: Path) -> list[dict[str, Any]]:
    """Every event of a record. A torn last line (a call cut off mid-write) is skipped."""
    events = []
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return events


def summarize(call_id: str, events: list[dict[str, Any]]) -> Summary:
    s = Summary(id=call_id)
    flags: set[str] = set()
    tools: list[str] = []
    booked = handed_off = locked = crashed = False
    book_ok_v1 = False
    for e in events:
        kind = e.get("kind")
        s.duration_ms = max(s.duration_ms, int(e.get("t_ms") or 0))
        if kind == "call_start":
            s.schema_version = int(e.get("schema_version") or 1)
            s.started = e.get("started")
            s.source = e.get("source") or "typed"
            s.mode = e.get("mode")
            s.model = e.get("model")
            s.scenario = e.get("scenario")
        elif kind == "node":
            if not s.reached or s.reached[-1] != e.get("name"):
                s.reached.append(e.get("name"))
        elif kind == "caller":
            s.turns += 1
        elif kind == "tool_call":
            tools.append(e.get("name"))
            if e.get("name") == "hand_off":
                handed_off = True
            if e.get("name") == "emergency":
                flags.add("emergency")
        elif kind == "ehr":
            result = e.get("result")
            if e.get("call") == "Book":
                if result == "booked":
                    booked = True
                elif result is None and e.get("outcome") == "ok":
                    book_ok_v1 = True  # schema 1 kept no result; see below
            if e.get("call") == "CheckCode" and result == "locked":
                locked = True
        elif kind == "screen":
            flags.add("screen_911")
        elif kind == "staff_flag":
            # A staff flag (FLAGS.md): the catalog's label names its kind, never its notes.
            flags.add(f"staff_{e.get('flag') or 'other'}")
        elif kind == "review_flag":
            # A review flag (FLAGS.md §8): for an analyst or engineer to look at the call.
            flags.add(f"review_{e.get('reason') or 'other'}")
        elif kind == "invented_value":
            flags.add("invented_value")
        elif kind == "words_before_tool":
            flags.add("words_before_tool")
        elif kind == "log" and e.get("level") in ("ERROR", "CRITICAL"):
            flags.add("library_error")
        elif kind == "error" or kind == "llm" and "error" in e:
            crashed = True
        elif kind == "call_end":
            s.end_reason = e.get("reason")
            tokens = e.get("tokens") or {}
            s.tokens_in = sum(
                int(tokens.get(k) or 0)
                for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
            )
            s.tokens_out = int(tokens.get("output_tokens") or 0)
            s.cost_usd = float(e.get("cost_usd") or 0.0)
    # Schema 1 kept no Book result. A Book that returned ok, whose tool result has no error,
    # was a booking: the fake scheduler's only other answer is "slot taken", which no
    # 2026-11-11 call reached.
    if book_ok_v1 and not booked:
        booked = any(
            e.get("kind") == "tool_result" and e.get("name") == "book" and not e.get("error")
            for e in events
        )
    s.tool_calls = len(tools)
    s.flags = sorted(flags)
    ended = s.end_reason is not None and "ended it" in s.end_reason
    if crashed or (s.end_reason or "").startswith(
        ("Claude's request failed", "the harness failed", "the EHR did not answer")
    ):
        s.outcome = "crashed"
    elif booked:
        s.outcome = "booked"
    elif handed_off:
        s.outcome = "handed_off"
    elif locked:
        s.outcome = "locked"
    elif ended or "end_call" in tools:
        s.outcome = "ended"
    else:
        s.outcome = "abandoned"
    return s


class AnnotationError(ValueError):
    """An annotation that does not fit the catalog. The message names the field, not a value."""


# The fields each kind takes, besides `kind` and `call`. Nothing else reaches the committed file
# (DEV-API.md §4). The console's `Annotation` type in console/src/agent/api.ts mirrors this
# (docs:check 14).
FIELDS = {
    "label": {"text"},
    "note": {"text"},
    "keep": {"value"},
    "rating": {"grader_kind", "grader", "dimension", "verdict", "score", "note"},
}
# Set by the catalog when it takes an annotation, never by the sender: they are on the file's
# lines, so a rebuild reads them back.
STAMPS = {"at", "rubric_version"}
MAX_TEXT = 2000
MAX_GRADER = 100
_FIELD_NAME = re.compile(r"^[a-z_]{1,40}$")


def check_annotation(a: dict[str, Any], *, stamped: bool = False) -> dict[str, Any]:
    """The one gate for every annotation, from the console or a grader. `stamped` is for the
    file's own lines, which carry the catalog's stamps. Returns a copy with only the kind's
    fields."""
    kind = a.get("kind")
    if kind not in FIELDS:
        raise AnnotationError("kind: not label, note, keep or rating")
    allowed = {"kind", "call"} | FIELDS[kind] | (STAMPS if stamped else set())
    for name in a:
        if name not in allowed:
            # The name only if it looks like a field name: a key is the sender's text.
            shown = name if isinstance(name, str) and _FIELD_NAME.match(name) else "a field"
            raise AnnotationError(f"{shown}: not a field of {kind}")
    if not isinstance(a.get("call"), str) or not a["call"]:
        raise AnnotationError("call: missing")
    if kind == "label" or kind == "note":
        if not isinstance(a.get("text"), str) or len(a["text"]) > MAX_TEXT:
            raise AnnotationError("text: missing or too long")
    elif kind == "keep":
        if not isinstance(a.get("value"), bool):
            raise AnnotationError("value: not true or false")
    elif kind == "rating":
        if a.get("grader_kind") not in GRADER_KINDS:
            raise AnnotationError("grader_kind: not human, model or code")
        if not isinstance(a.get("grader"), str) or not a["grader"] or len(a["grader"]) > MAX_GRADER:
            raise AnnotationError("grader: missing or too long")
        if a.get("dimension") not in DIMENSIONS:
            raise AnnotationError("dimension: not in the rubric")
        if a.get("verdict") not in VERDICTS:
            raise AnnotationError("verdict: not pass, fail or unknown")
        score = a.get("score")
        if score is not None and (not isinstance(score, int) or isinstance(score, bool) or not 0 <= score <= 5):
            raise AnnotationError("score: not a whole number from 0 to 5")
        if a.get("note") is not None and (not isinstance(a["note"], str) or len(a["note"]) > MAX_TEXT):
            raise AnnotationError("note: too long")
    if stamped:
        for name in STAMPS & a.keys():
            if not isinstance(a[name], str):
                raise AnnotationError(f"{name}: not text")
    return {name: a[name] for name in a if name in allowed}


class Catalog:
    def __init__(self, folder: Path = CALLS) -> None:
        self.folder = folder
        self.db_path = folder / DATABASE
        self.annotations_path = folder / ANNOTATIONS
        self.db: sqlite3.Connection | None = None

    def build(self) -> Catalog:
        """Rebuild the database from the records and the annotations."""
        self.folder.mkdir(parents=True, exist_ok=True)
        if self.db is not None:
            self.db.close()
        self.db_path.unlink(missing_ok=True)
        self.db = sqlite3.connect(self.db_path, check_same_thread=False)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(SCHEMA)
        for path in sorted(self.folder.glob("*.jsonl")):
            if path.name == ANNOTATIONS:
                continue
            events = read_events(path)
            self._put(summarize(path.stem, events))
            self._put_tools(path.stem, events)
        if self.annotations_path.exists():
            for line in self.annotations_path.read_text(encoding="utf-8").splitlines():
                try:
                    line_data = json.loads(line)
                    if not isinstance(line_data, dict):
                        continue
                    self._apply(check_annotation(line_data, stamped=True))
                except (json.JSONDecodeError, AnnotationError):
                    continue  # a bad line never stops the catalog; the file keeps it for a person
        self.db.commit()
        return self

    def close(self) -> None:
        """Let go of the database. On Windows another catalog cannot rebuild it while it is open."""
        if self.db is not None:
            self.db.close()
            self.db = None

    def refresh(self, call_id: str) -> None:
        """Re-read one record (a call that just ended) without a full rebuild."""
        path = self.folder / f"{call_id}.jsonl"
        if self.db is None or not path.exists():
            return
        old = self.db.execute("SELECT label, note, keep FROM calls WHERE id = ?", (call_id,)).fetchone()
        self.db.execute("DELETE FROM tool_calls WHERE call_id = ?", (call_id,))
        self.db.execute("DELETE FROM calls WHERE id = ?", (call_id,))
        events = read_events(path)
        self._put(summarize(call_id, events))
        self._put_tools(call_id, events)
        if old is not None:
            self.db.execute(
                "UPDATE calls SET label = ?, note = ?, keep = ? WHERE id = ?",
                (old["label"], old["note"], old["keep"], call_id),
            )
        self.db.commit()

    def annotate(self, annotation: dict[str, Any]) -> None:
        """Check, stamp, append to the committed file, then apply. The stamps are the catalog's
        own: a sender's `at` or `rubric_version` is refused by the gate."""
        a = check_annotation(annotation)
        a["at"] = datetime.now().isoformat(timespec="seconds")
        if a["kind"] == "rating":
            a["rubric_version"] = RUBRIC_VERSION
        if self.db is None or self.db.execute("SELECT 1 FROM calls WHERE id = ?", (a["call"],)).fetchone() is None:
            raise AnnotationError("call: no such call")
        with self.annotations_path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(a, ensure_ascii=False) + "\n")
        self._apply(a)
        self.db.commit()

    def list_calls(self) -> list[dict[str, Any]]:
        """Every call, starred first, then newest first, with its current ratings."""
        assert self.db is not None
        rows = self.db.execute("SELECT * FROM calls ORDER BY keep DESC, id DESC").fetchall()
        ratings: dict[str, list[dict[str, Any]]] = {}
        for r in self.db.execute("SELECT * FROM current_ratings ORDER BY id"):
            ratings.setdefault(r["call_id"], []).append(_rating(r))
        return [_call(r, ratings.get(r["id"], [])) for r in rows]

    def events(self, call_id: str) -> list[dict[str, Any]] | None:
        """A record's events, or None. The id must name a record in the catalog, so it can never
        reach outside the folder."""
        assert self.db is not None
        if self.db.execute("SELECT 1 FROM calls WHERE id = ?", (call_id,)).fetchone() is None:
            return None
        return read_events(self.folder / f"{call_id}.jsonl")

    def latency(
        self,
        source: str | None = None,
        model: str | None = None,
        since: str | None = None,
        exact: bool = False,
        with_replayed: bool = False,
        rows: bool = False,
    ) -> dict[str, Any]:
        """Per-tool latency (latency.py): a summary per tool and model, and the rows if asked.
        `since` is a day (YYYY-MM-DD) or a call id; calls are named by their start time."""
        assert self.db is not None
        where, args = ["1 = 1"], []
        if source:
            where.append("c.source = ?")
            args.append(source)
        if model:
            where.append("c.model = ?")
            args.append(model)
        if since:
            # A day "2026-11-12" becomes "20261112", the start of every id made that day.
            day = re.fullmatch(r"(\d{4})-(\d{2})-(\d{2})", since)
            where.append("c.id >= ?")
            args.append("".join(day.groups()) if day else since)
        if exact:
            where.append("t.exact = 1")
        found = [
            _tool_row(r)
            for r in self.db.execute(
                "SELECT t.*, c.model AS model, c.source AS source FROM tool_calls t"
                " JOIN calls c ON c.id = t.call_id WHERE " + " AND ".join(where) + " ORDER BY t.call_id, t.seq",
                args,
            )
        ]
        out: dict[str, Any] = {
            "filters": {"source": source, "model": model, "since": since, "exact": exact, "with_replayed": with_replayed},
            "spans": list(latency.SPANS),
            "summary": latency.summarize(found, with_replayed=with_replayed),
        }
        if rows:
            out["rows"] = [r for r in found if with_replayed or not r["replayed"]]
        return out

    # ---- inside ----

    def _put_tools(self, call_id: str, events: list[dict[str, Any]]) -> None:
        assert self.db is not None
        for r in latency.tool_rows(call_id, events):
            self.db.execute(
                "INSERT OR REPLACE INTO tool_calls (call_id, seq, t_ms, tool, node, trigger_ms, model_ms,"
                " confirm_ms, ehr_ms, ehr_calls, answer_ms, total_ms, replayed, exact, failed)"
                " VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    call_id, r["seq"], r["t_ms"], r["tool"] or "", r["node"], r["trigger_ms"], r["model_ms"],
                    r["confirm_ms"], r["ehr_ms"], json.dumps(r["ehr_calls"]), r["answer_ms"], r["total_ms"],
                    int(r["replayed"]), int(r["exact"]), int(r["failed"]),
                ),
            )

    def _put(self, s: Summary) -> None:
        assert self.db is not None
        self.db.execute(
            "INSERT OR REPLACE INTO calls (id, schema_version, started, source, mode, model, scenario,"
            " outcome, end_reason, reached, turns, tool_calls, duration_ms, tokens_in, tokens_out,"
            " cost_usd, flags) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                s.id, s.schema_version, s.started, s.source, s.mode, s.model, s.scenario,
                s.outcome, s.end_reason, json.dumps(s.reached), s.turns, s.tool_calls,
                s.duration_ms, s.tokens_in, s.tokens_out, s.cost_usd, json.dumps(s.flags),
            ),
        )

    def _apply(self, a: dict[str, Any]) -> None:
        assert self.db is not None
        kind, call = a["kind"], a["call"]
        if kind == "label":
            self.db.execute("UPDATE calls SET label = ? WHERE id = ?", (a["text"] or None, call))
        elif kind == "note":
            self.db.execute("UPDATE calls SET note = ? WHERE id = ?", (a["text"] or None, call))
        elif kind == "keep":
            self.db.execute("UPDATE calls SET keep = ? WHERE id = ?", (int(a["value"]), call))
        elif kind == "rating":
            if self.db.execute("SELECT 1 FROM calls WHERE id = ?", (call,)).fetchone() is None:
                return  # a rating for a record this machine does not have
            self.db.execute(
                "INSERT INTO ratings (call_id, grader_kind, grader, dimension, verdict, score, note,"
                " rubric_version, rated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    call, a["grader_kind"], a["grader"], a["dimension"], a["verdict"],
                    a.get("score"), a.get("note"), a.get("rubric_version") or RUBRIC_VERSION,
                    a.get("at") or "",
                ),
            )


def _call(r: sqlite3.Row, ratings: list[dict[str, Any]]) -> dict[str, Any]:
    d = dict(r)
    d["reached"] = json.loads(d["reached"])
    d["flags"] = json.loads(d["flags"])
    d["keep"] = bool(d["keep"])
    d["ratings"] = ratings
    return d


def _tool_row(r: sqlite3.Row) -> dict[str, Any]:
    d = dict(r)
    d["ehr_calls"] = json.loads(d["ehr_calls"])
    for k in ("replayed", "exact", "failed"):
        d[k] = bool(d[k])
    return d


def _rating(r: sqlite3.Row) -> dict[str, Any]:
    return {k: r[k] for k in ("grader_kind", "grader", "dimension", "verdict", "score", "note", "rubric_version", "rated_at")}
