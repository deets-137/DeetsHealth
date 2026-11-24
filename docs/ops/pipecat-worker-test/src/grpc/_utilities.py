"""Stand-in: the generated code's version check."""


def first_version_is_lower(a: str, b: str) -> bool:
    def key(v: str) -> tuple[int, ...]:
        return tuple(int(x) for x in v.split(".")[:3])

    return key(a) < key(b)
