"""Stand-in for the native `sentencex` package (no Pyodide build): a plain sentence splitter."""

import re

_END = re.compile(r"(?<=[.!?])\s+")


def segment(language: str, text: str):
    start = 0
    for m in _END.finditer(text):
        yield text[start : m.end()]
        start = m.end()
    if start < len(text):
        yield text[start:]
