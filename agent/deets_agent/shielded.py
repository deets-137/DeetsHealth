"""The agent's wrapper for protected data (SECURITY.md §4.2; EHR-SERVICE.md §5).

The Python twin of core's `Shielded<T>`. It prints as `***` in every form Python has: `str`,
`repr`, an f-string, `%` formatting, a log line, a traceback's locals. It refuses pickling and
JSON. The value comes out only through `reveal()`, so every place that opens one is a grep
away.

Python cannot wipe a string from memory, so unlike the Rust type this one cannot zeroize. The
agent holds a protected value only for the length of one call.
"""

from __future__ import annotations

from typing import NoReturn

_MASK = "***"


class Shielded[T]:
    __slots__ = ("_value",)

    def __init__(self, value: T) -> None:
        self._value = value

    def reveal(self) -> T:
        """The plaintext. Only the code that hands words to Claude or to the caller, and the
        edge that builds a request to ehr, may call this."""
        return self._value

    def __repr__(self) -> str:
        return _MASK

    __str__ = __repr__

    def __format__(self, spec: str) -> str:
        return _MASK

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Shielded) and self._value == other._value

    __hash__ = None  # type: ignore[assignment]

    def __bool__(self) -> bool:
        return bool(self._value)

    def __reduce__(self) -> NoReturn:
        raise TypeError("a Shielded value cannot be pickled")

    def __getstate__(self) -> NoReturn:
        raise TypeError("a Shielded value cannot be serialized")
