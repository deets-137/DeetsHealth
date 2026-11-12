"""The catalog of dev call records (catalog.py): rows worked out from events, annotations kept
in the committed file, the latest rating winning, and the gate on every annotation."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import pytest

from deets_agent.call_record import CALLS
from deets_agent.catalog import AnnotationError, Catalog, check_annotation, summarize


def _write(folder: Path, name: str, events: list[dict]) -> None:
    (folder / f"{name}.jsonl").write_text("\n".join(json.dumps(e) for e in events) + "\n", encoding="utf-8")


BOOKED = [
    {"t_ms": 0, "kind": "call_start", "schema_version": 2, "call_id": "a", "source": "test",
     "scenario": "book_a_visit", "mode": "replay", "model": "m", "started": "2026-11-12T10:00:00"},
    {"t_ms": 5, "kind": "node", "name": "greet"},
    {"t_ms": 6, "kind": "caller", "text": "hi"},
    {"t_ms": 7, "kind": "node", "name": "booking"},
    {"t_ms": 8, "kind": "tool_call", "name": "book", "arguments": {}},
    {"t_ms": 9, "kind": "ehr", "call": "Book", "ms": 1, "outcome": "ok", "result": "booked"},
    {"t_ms": 900, "kind": "call_end", "reason": "the agent ended it", "cost_usd": 0.01,
     "tokens": {"input_tokens": 3, "output_tokens": 4, "cache_read_input_tokens": 10}},
]


def test_a_row_comes_from_the_events():
    s = summarize("a", BOOKED)
    assert (s.outcome, s.source, s.scenario, s.schema_version) == ("booked", "test", "book_a_visit", 2)
    assert s.reached == ["greet", "booking"]
    assert (s.turns, s.tool_calls, s.duration_ms, s.tokens_in, s.tokens_out) == (1, 1, 900, 13, 4)


def test_the_outcome_is_the_real_state_not_the_words():
    # The agent said "booked" but ehr answered slot_taken: not a booking.
    taken = [e if e.get("call") != "Book" else {**e, "result": "slot_taken"} for e in BOOKED]
    assert summarize("a", taken).outcome == "ended"


def test_a_failed_request_is_a_crash():
    events = [*BOOKED[:3], {"t_ms": 4, "kind": "llm", "error": {"type": "ReplayMiss"}}]
    assert summarize("a", events).outcome == "crashed"


def test_annotations_survive_a_rebuild_and_the_latest_rating_wins(tmp_path):
    _write(tmp_path, "a", BOOKED)
    c = Catalog(tmp_path).build()
    c.annotate({"kind": "label", "call": "a", "text": "clean booking"})
    c.annotate({"kind": "keep", "call": "a", "value": True})
    c.annotate({"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner",
                "dimension": "intent_drift", "verdict": "fail", "score": 1})
    c.annotate({"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner",
                "dimension": "intent_drift", "verdict": "pass", "score": 4})
    c.annotate({"kind": "rating", "call": "a", "grader_kind": "model", "grader": "claude-haiku-5-5",
                "dimension": "intent_drift", "verdict": "pass"})
    c.close()
    row = Catalog(tmp_path).build().list_calls()[0]
    assert (row["label"], row["keep"]) == ("clean booking", True)
    drift = {r["grader"]: (r["verdict"], r["score"]) for r in row["ratings"]}
    assert drift == {"owner": ("pass", 4), "claude-haiku-5-5": ("pass", None)}


@pytest.mark.parametrize(
    "bad",
    [
        {"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner", "dimension": "x", "verdict": "pass"},
        {"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner", "dimension": "phone_style", "verdict": "pass", "score": 6},
        {"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner", "dimension": "phone_style", "verdict": "pass", "score": True},
        {"kind": "rating", "call": "a", "grader_kind": "boss", "grader": "owner", "dimension": "phone_style", "verdict": "pass"},
        {"kind": "keep", "call": "a", "value": "yes"},
        {"kind": "label", "call": "nope", "text": "x"},
        {"kind": "delete", "call": "a"},
    ],
)
def test_the_gate_refuses_what_does_not_fit(tmp_path, bad):
    _write(tmp_path, "a", BOOKED)
    c = Catalog(tmp_path).build()
    with pytest.raises(AnnotationError):
        c.annotate(bad)
    assert not (tmp_path / "annotations.jsonl").exists()


def test_events_never_reach_outside_the_folder(tmp_path):
    _write(tmp_path, "a", BOOKED)
    c = Catalog(tmp_path).build()
    assert c.events("../a") is None
    assert c.events("a")[0]["kind"] == "call_start"


def test_every_kept_annotation_passes_the_gate():
    path = CALLS / "annotations.jsonl"
    if not path.exists():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        check_annotation(json.loads(line), stamped=True)


def test_a_sender_never_sets_the_stamps(tmp_path):
    _write(tmp_path, "a", BOOKED)
    c = Catalog(tmp_path).build()
    with pytest.raises(AnnotationError):
        c.annotate({"kind": "keep", "call": "a", "value": True, "at": "2020-01-01T00:00:00"})
    c.annotate({"kind": "keep", "call": "a", "value": True})
    line = json.loads((tmp_path / "annotations.jsonl").read_text(encoding="utf-8"))
    assert set(line) == {"kind", "call", "value", "at"}


def test_every_kept_record_reads(tmp_path):
    for p in CALLS.glob("*.jsonl"):
        if p.name != "annotations.jsonl":
            shutil.copy(p, tmp_path)
    rows = Catalog(tmp_path).build().list_calls()
    assert len(rows) == len(list(tmp_path.glob("*.jsonl")))
    assert all(r["outcome"] for r in rows)
