"""Cloudflare's Access group for the demo (docs/ops/DEMO.md §3.5).

The store's People list is the record (ehr's users.rs); this keeps the Access group that lets
people sign in the same as the list. Three calls to Cloudflare's API:

- read the group's emails (to bring in people added in Cloudflare's dashboard);
- write the group's whole list of emails (after each add or remove);
- end one person's Access sessions (after a remove), so a removed person is out at once, not
  when their session ends. The agent's own check (access.py) also refuses them within 30 s.

The API token sits in `.env` (`DEETS_CF_API_TOKEN`) and never reaches a browser. Cloudflare
cannot narrow a token to one group, so this code touches only the one group `.env` names.
Nothing here logs an email; an error names the step and Cloudflare's status, never a value.
"""

from __future__ import annotations

import os
import re
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

import aiohttp

API = "https://api.cloudflare.com/client/v4"

Request = Callable[[str, str, dict[str, str], dict[str, Any] | None], Awaitable[tuple[int, Any]]]


@dataclass(frozen=True)
class CloudflareSettings:
    token: str
    account: str
    group: str

    @classmethod
    def from_env(cls) -> CloudflareSettings | None:
        """The token, account and group from `.env`, or None: then the People list still works
        in the store, and the page says Cloudflare is not linked."""
        token = os.environ.get("DEETS_CF_API_TOKEN", "").strip()
        account = os.environ.get("DEETS_CF_ACCOUNT_ID", "").strip()
        group = os.environ.get("DEETS_ACCESS_GROUP_ID", "").strip()
        if not (token and account and group):
            return None
        if not re.fullmatch(r"[0-9a-f]{32}", account) or not re.fullmatch(r"[0-9a-f-]{36}", group):
            raise ValueError("DEETS_CF_ACCOUNT_ID is 32 hex digits; DEETS_ACCESS_GROUP_ID a uuid")
        return cls(token=token, account=account, group=group)


class CloudflareError(RuntimeError):
    """A call to Cloudflare failed. The message names the step and the status only."""


async def _request(method: str, url: str, headers: dict[str, str], body: dict[str, Any] | None):
    timeout = aiohttp.ClientTimeout(total=15)
    async with (
        aiohttp.ClientSession(timeout=timeout) as session,
        session.request(method, url, headers=headers, json=body) as response,
    ):
        return response.status, await response.json(content_type=None)


class AccessGroup:
    def __init__(self, settings: CloudflareSettings, request: Request = _request) -> None:
        self.settings = settings
        self._request = request

    def _url(self, path: str) -> str:
        return f"{API}/accounts/{self.settings.account}/access/{path}"

    async def _call(self, step: str, method: str, path: str, body: dict[str, Any] | None = None):
        headers = {"Authorization": f"Bearer {self.settings.token}"}
        try:
            status, data = await self._request(method, self._url(path), headers, body)
        except (aiohttp.ClientError, TimeoutError):
            raise CloudflareError(f"{step}: Cloudflare did not answer") from None
        if status != 200 or not isinstance(data, dict) or not data.get("success"):
            raise CloudflareError(f"{step}: Cloudflare answered {status}")
        return data.get("result")

    async def _group(self) -> dict[str, Any]:
        group = await self._call("read the group", "GET", f"groups/{self.settings.group}")
        if not isinstance(group, dict):
            raise CloudflareError("read the group: no group in the answer")
        return group

    async def emails(self) -> list[str]:
        """The emails the group lets in now, lower case."""
        group = await self._group()
        out = []
        for rule in group.get("include") or []:
            email = (rule.get("email") or {}).get("email") if isinstance(rule, dict) else None
            if isinstance(email, str):
                out.append(email.strip().lower())
        return out

    async def set_emails(self, emails: list[str]) -> None:
        """Make the group let in exactly these emails. Its other rules (exclude, require) stay
        as they are. Refuses an empty list: the owner is always on it."""
        if not emails:
            raise CloudflareError("write the group: an empty list would lock everyone out")
        group = await self._group()
        body = {
            "name": group.get("name") or "DeetsHealth demo guests",
            "include": [{"email": {"email": e}} for e in sorted(set(emails))],
            "exclude": group.get("exclude") or [],
            "require": group.get("require") or [],
        }
        await self._call("write the group", "PUT", f"groups/{self.settings.group}", body)

    async def end_sessions(self, email: str) -> None:
        """End this person's Access sessions everywhere."""
        await self._call("end the sessions", "POST", "organizations/revoke_user", {"email": email})
