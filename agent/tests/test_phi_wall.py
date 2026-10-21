"""The PHI wall on the agent's side: the wrapper, the redacted wire text, the logs."""

import json
import logging
import pickle

import pytest
from loguru import logger

from deets_agent import redact
from deets_agent.proto import pb
from deets_agent.shielded import Shielded


def test_shielded_never_prints_its_value():
    s = Shielded("CANARY")
    # Every way Python turns a value into text, the old `%` form too.
    for text in (repr(s), str(s), f"{s}", f"{s!r}", "%s" % s, str([s]), str({"k": s})):  # noqa: UP031
        assert "CANARY" not in text
    assert s.reveal() == "CANARY"


def test_shielded_refuses_pickle_and_json():
    with pytest.raises(TypeError):
        pickle.dumps(Shielded("CANARY"))
    with pytest.raises(TypeError):
        json.dumps({"v": Shielded("CANARY")})


def test_redacted_text_blanks_every_marked_field():
    msg = pb.FindSlotsReply(
        slots=[pb.Slot(token="CANARY", starts_at="CANARY", minutes=20, clinician="CANARY", place="CANARY")]
    )
    text = redact.redacted_text(msg)
    assert "CANARY" not in text and "minutes=20" in text
    # Python's own text form leaks; that is why the redactor exists.
    assert "CANARY" in str(msg)


@pytest.fixture
def captured():
    redact.install("DEBUG")
    lines: list[str] = []
    sink = logger.add(lines.append, format="{name} {message}", level="DEBUG")
    yield lines
    logger.remove(sink)


def test_library_log_text_is_withheld(captured):
    logging.getLogger("grpc._channel").warning("request for CANARY failed")
    logger.bind(lib_name="pipecat.services.llm").debug("context: CANARY")
    assert captured and all("CANARY" not in line for line in captured)
    assert all(redact.WITHHELD in line for line in captured)


def test_an_exception_keeps_its_type_and_drops_its_message(captured):
    try:
        raise ValueError("CANARY")
    except ValueError:
        logger.exception("something failed")
    assert "CANARY" not in "".join(captured)
    assert "ValueError" in "".join(captured)
