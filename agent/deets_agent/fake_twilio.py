"""The fake Twilio: the one part of Twilio's Messages API that ehr calls, inside the agent's dev
server (CHAT.md §8; VOICE-AGENT.md §9, the fake Twilio).

ehr sends the code by text with the same request it will send Twilio from Phase 5
(ehr/src/texts.rs): `POST /2010-04-01/Accounts/{AccountSid}/Messages.json`, HTTP Basic auth with
the account and its auth token, a form with `To`, `MessagingServiceSid` and `Body`. This fake
checks each part as Twilio would, answers 201 with a message id, and hands the text to the dev
server, which writes it into the live call's record. The Chat page shows it as the code button.

**Dev only, synthetic only.** Its account, token and sender are made fresh at each launch and
handed to ehr through its environment; they mean nothing to Twilio. It answers only on
127.0.0.1, only with that account's Basic auth, and never to a browser (a request with an
`Origin` is refused).
"""

from __future__ import annotations

import base64
import hmac
import re
import secrets
from dataclasses import dataclass, field
from urllib.parse import parse_qs

E164 = re.compile(r"^\+[1-9]\d{6,14}$")
# One text segment in GSM-7. Twilio splits a longer one; the fake refuses it, so the words in
# AGENT-PROMPT.md › text.code never grow past one text unseen.
MAX_BODY = 160


def _sid(prefix: str) -> str:
    return prefix + secrets.token_hex(16)


@dataclass
class FakeTwilio:
    port: int
    account_sid: str = field(default_factory=lambda: _sid("AC"))
    auth_token: str = field(default_factory=lambda: secrets.token_hex(32))
    messaging_service: str = field(default_factory=lambda: _sid("MG"))

    def ehr_env(self) -> dict[str, str]:
        """The settings ehr reads (ehr/src/texts.rs)."""
        return {
            "DEETS_TWILIO_API": f"http://127.0.0.1:{self.port}",
            "DEETS_TWILIO_ACCOUNT_SID": self.account_sid,
            "DEETS_TWILIO_AUTH_TOKEN": self.auth_token,
            "DEETS_TWILIO_MESSAGING_SERVICE": self.messaging_service,
        }

    def path(self) -> str:
        return f"/2010-04-01/Accounts/{self.account_sid}/Messages.json"

    def authorized(self, headers, client_host: str | None) -> bool:
        """This machine, no browser, and this account's Basic auth, compared in constant time."""
        if client_host not in ("127.0.0.1", "::1") or headers.get("origin") is not None:
            return False
        want = base64.b64encode(f"{self.account_sid}:{self.auth_token}".encode()).decode()
        got = headers.get("authorization", "")
        return hmac.compare_digest(got.encode(), f"Basic {want}".encode())

    def take(self, raw: bytes) -> tuple[int, dict]:
        """Checks one Messages request's form. Returns the status and Twilio's reply, and, on
        201, the text under `_text` for the dev server (removed before the reply goes out)."""
        form = {k: v[0] for k, v in parse_qs(raw.decode("utf-8", "replace"), keep_blank_values=True).items()}
        to, body, sender = form.get("To", ""), form.get("Body", ""), form.get("MessagingServiceSid", "")
        if sender != self.messaging_service:
            return 400, _error(21705, "The Messaging Service Sid is invalid.")
        if not E164.match(to):
            return 400, _error(21211, "The 'To' number is not a valid phone number.")
        if not body.strip():
            return 400, _error(21602, "Message body is required.")
        if len(body) > MAX_BODY or not body.isascii():
            return 400, _error(30000, "Fake Twilio: the text is longer than one segment.")
        reply = {
            "sid": _sid("SM"),
            "account_sid": self.account_sid,
            "messaging_service_sid": sender,
            "status": "accepted",
            "num_segments": "1",
            "_text": {"to": to, "body": body},
        }
        return 201, reply


def _error(code: int, message: str) -> dict:
    return {"code": code, "message": message, "status": 400}
