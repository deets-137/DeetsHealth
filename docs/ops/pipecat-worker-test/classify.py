"""Classify every package Pipecat needs: pure Python, native with a Pyodide build, or native without.

Usage: python classify.py <uv.lock> <pyodide-lock.json>
"""

import json
import sys
import tomllib

lock = tomllib.load(open(sys.argv[1], "rb"))
pyodide = json.load(open(sys.argv[2], encoding="utf-8"))["packages"]
pyodide_names = {k.lower().replace("_", "-") for k in pyodide}

pkgs = {p["name"]: p for p in lock["package"]}


def deps_of(name: str, extras: tuple[str, ...] = ()) -> list[str]:
    p = pkgs[name]
    out = [d["name"] for d in p.get("dependencies", []) if "sys_platform == 'win32'" not in d.get("marker", "")]
    for e in extras:
        out += [d["name"] for d in p.get("optional-dependencies", {}).get(e, [])]
    return out


seen: set[str] = set()
todo = [("pipecat-ai", ("anthropic",)), ("pipecat-ai-flows", ())]
while todo:
    name, extras = todo.pop()
    if name in seen or name not in pkgs:
        continue
    seen.add(name)
    todo += [(d, ()) for d in deps_of(name, extras)]

pure, native_ok, native_missing = [], [], []
for name in sorted(seen):
    wheels = [w["url"].rsplit("/", 1)[-1] for w in pkgs[name].get("wheels", [])]
    if any(w.endswith("-none-any.whl") for w in wheels):
        pure.append(name)
    elif any("emscripten" in w or "pyodide" in w for w in wheels) or name in pyodide_names:
        native_ok.append(name)
    else:
        native_missing.append(name)

print(f"packages: {len(seen)}")
print(f"pure Python ({len(pure)}): {', '.join(pure)}")
print(f"native, Pyodide has a build ({len(native_ok)}): {', '.join(native_ok)}")
print(f"native, no Pyodide build ({len(native_missing)}): {', '.join(native_missing)}")
