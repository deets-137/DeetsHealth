"""The fake Twilio (deets_agent/fake_twilio.py): it checks what Twilio checks, and only ehr gets in."""

from __future__ import annotations

import base64
from urllib.parse import urlencode

from deets_agent.fake_twilio import FakeTwilio


def _auth(t: FakeTwilio, token: str | None = None) -> dict[str, str]:
    raw = f"{t.account_sid}:{token or t.auth_token}".encode()
    return {"authorization": "Basic " + base64.b64encode(raw).decode()}


def _form(t: FakeTwilio, **over: str) -> bytes:
    fields = {"To": "+15555550101", "MessagingServiceSid": t.messaging_service, "Body": "Your code is 123456."}
    fields.update(over)
    return urlencode(fields).encode()


def test_only_this_machine_with_the_account_and_no_browser():
    t = FakeTwilio(9000)
    assert t.authorized(_auth(t), "127.0.0.1")
    assert not t.authorized(_auth(t), "10.0.0.5"), "another machine"
    assert not t.authorized(_auth(t, "wrong"), "127.0.0.1"), "a wrong token"
    assert not t.authorized({}, "127.0.0.1"), "no auth"
    assert not t.authorized({**_auth(t), "origin": "http://localhost:1420"}, "127.0.0.1"), "a browser"


def test_a_good_text_is_taken_and_handed_on():
    t = FakeTwilio(9000)
    status, reply = t.take(_form(t))
    assert status == 201
    assert reply["sid"].startswith("SM") and len(reply["sid"]) == 34
    assert reply["_text"] == {"to": "+15555550101", "body": "Your code is 123456."}


def test_what_twilio_refuses_the_fake_refuses():
    t = FakeTwilio(9000)
    assert t.take(_form(t, MessagingServiceSid="MG" + "0" * 32))[0] == 400
    assert t.take(_form(t, To="5550101"))[0] == 400
    assert t.take(_form(t, Body=""))[0] == 400
    assert t.take(_form(t, Body="x" * 161))[0] == 400, "longer than one text"


def test_ehr_gets_settings_that_point_here():
    t = FakeTwilio(9123)
    env = t.ehr_env()
    assert env["DEETS_TWILIO_API"] == "http://127.0.0.1:9123"
    assert t.path() == f"/2010-04-01/Accounts/{env['DEETS_TWILIO_ACCOUNT_SID']}/Messages.json"
