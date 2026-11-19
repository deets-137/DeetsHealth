"""Who a request is, and what they may do (docs/ideas/ACCESS-ROLES.md §5, §6 Hand-over 2).

The seam for staff sign-in: one interface, `Identity.who(headers)`, and one source per way in.
Each source turns a request into a person on the People list (their user id); ehr then gives
that person's levels (the highest of their live roles on each lever, access.rs). Staff sign-in
(OIDC, STAFF-SIGN-IN.md M2) is a new source here; the routes, ehr and the console do not change.

The sources now:
- **The launch token** (the owner's PC): the owner's row, the person the launch token speaks
  for (ehr's `SeenOwner`, which also stamps last seen, at most once a minute).
- **Cloudflare Access** (the demo's second door, access.py): the guest's email, then their row
  (ehr's `AdmitUser`).

Levels are read from ehr and kept `LEVELS_TTL` seconds; a grant, a revoke or a roles save made
through this server drops them at once (`forget`).

**No roles store.** Without ehr or without its store (no Postgres) there are no people and no
roles. Then the launch token is the owner with Edit on every lever, as before roles, and a guest
gets nothing. Dev only: a deploy always has its store.

Never logged: the email, the user id.
"""

from __future__ import annotations

import time
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

from deets_agent.ehr_client import ACTOR, EhrError, ErrorKind

if TYPE_CHECKING:
    from deets_agent.dev_server import State

# The levers and levels, in access.rs's words and order.
LEVERS = ("phi", "auth", "roles", "people", "agent", "vendors", "rules", "testing", "analytics", "flags", "audit_logs")
LEVELS = ("none", "view", "edit")
# How long a person's levels and a guest's id are kept before ehr is asked again.
LEVELS_TTL = 30.0
# How often the launch token's requests stamp the owner's last seen (and read the owner's id).
OWNER_EVERY = 60.0


def rank(level: str) -> int:
    return LEVELS.index(level) if level in LEVELS else 0


@dataclass(frozen=True)
class Principal:
    """One request's person. `door`: `launch` or `access`. `user_id`: their row (None without a
    roles store). `email`: a guest's, kept in memory only (the Chat rail's list of calls)."""

    door: str
    user_id: str | None
    levels: Mapping[str, str]
    email: str | None = None
    role_ids: tuple[str, ...] = ()
    roles_version: int = 0
    store: bool = True
    extra: dict[str, Any] = field(default_factory=dict)

    def may(self, lever: str, level: str) -> bool:
        return rank(self.levels.get(lever, "none")) >= rank(level)

    @property
    def owner(self) -> bool:
        """Through the launch token: the owner's PC."""
        return self.door == "launch"

    def view(self) -> dict[str, Any]:
        """`GET /agent/me`: the id, the roles and every lever's level. Never the email."""
        return {
            "user_id": self.user_id,
            "door": self.door,
            "role_ids": list(self.role_ids),
            "roles_version": self.roles_version,
            "levels": {lever: self.levels.get(lever, "none") for lever in LEVERS},
            "store": self.store,
        }


ALL_EDIT = {lever: "edit" for lever in LEVERS}
NONE = {lever: "none" for lever in LEVERS}


class IdentityDown(Exception):
    """ehr did not answer who the person is: the request is refused with 503, never let in."""


class Identity:
    def __init__(self, state: State) -> None:
        self.state = state
        self._owner: tuple[float, str | None] | None = None
        self._guests: dict[str, tuple[float, str | None]] = {}
        self._levels: dict[str, tuple[float, dict[str, Any]]] = {}

    async def who(self, headers: Mapping[str, str]) -> Principal | None:
        """The request's person, or None when no door lets it in (or the person is not on the
        list). Raises IdentityDown when ehr does not answer."""
        if self.state.allowed(headers):
            door, email = "launch", None
        elif self.state.access is not None and (email := await self.state.access.email(headers)) is not None:
            door = "access"
        else:
            return None
        ehr = self.state.ehr
        if ehr is None:
            return self._no_store(door, email)
        try:
            user_id = await (self._owner_id() if door == "launch" else self._guest_id(email or ""))
            if user_id is None:
                # The launch token with no owner row yet (a new machine with no Cloudflare
                # group): the owner's PC, as with no store. A guest not on the list: no entry.
                return self._no_store(door, email) if door == "launch" else None
            found = await self._levels_of(user_id)
        except EhrError as e:
            if e.kind is ErrorKind.NOT_ALLOWED_NOW:
                return self._no_store(door, email)
            if e.kind is ErrorKind.FORBIDDEN:
                return None
            raise IdentityDown from None
        return Principal(
            door=door,
            user_id=user_id,
            levels=found["levels"],
            email=email,
            role_ids=tuple(found["role_ids"]),
            roles_version=found["roles_version"],
        )

    def forget(self, user_id: str | None = None) -> None:
        """Drop the kept levels (one person's, or everyone's after a roles save), so the next
        request asks ehr."""
        if user_id is None:
            self._levels.clear()
        else:
            self._levels.pop(user_id, None)

    def forget_guest(self, email: str) -> None:
        self._guests.pop(email.strip().lower(), None)

    @staticmethod
    def _no_store(door: str, email: str | None) -> Principal:
        return Principal(door=door, user_id=None, levels=ALL_EDIT if door == "launch" else NONE, email=email, store=False)

    async def _owner_id(self) -> str | None:
        now = time.monotonic()
        if self._owner is not None and now - self._owner[0] < OWNER_EVERY:
            return self._owner[1]
        assert self.state.ehr is not None
        owner = await self.state.ehr.seen_owner()
        self._owner = (now, owner)
        return owner

    async def _guest_id(self, email: str) -> str | None:
        now = time.monotonic()
        kept = self._guests.get(email)
        if kept is not None and now - kept[0] < LEVELS_TTL:
            return kept[1]
        assert self.state.ehr is not None
        found = await self.state.ehr.admit_user(email)
        self._guests[email] = (now, found)
        return found

    async def _levels_of(self, user_id: str) -> dict[str, Any]:
        now = time.monotonic()
        kept = self._levels.get(user_id)
        if kept is not None and now - kept[0] < LEVELS_TTL:
            return kept[1]
        assert self.state.ehr is not None
        token = ACTOR.set(user_id)
        try:
            found = await self.state.ehr.levels()
        finally:
            ACTOR.reset(token)
        self._levels[user_id] = (now, found)
        return found
