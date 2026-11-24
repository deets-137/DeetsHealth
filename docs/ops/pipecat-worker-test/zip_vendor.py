"""Pack the two SDKs with thousands of small files into one zip the Worker imports from.

The Workers upload API refused a bundle of 7,496 files ("multipart: message too large").
Usage: python zip_vendor.py <worker folder>
"""

import sys
import zipfile
from pathlib import Path

root = Path(sys.argv[1])
out = root / "src" / "vendor.zip"
count = 0
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for pkg in ("anthropic", "openai"):
        for p in sorted((root / "python_modules" / pkg).rglob("*")):
            if p.is_file() and p.suffix not in (".pyc", ".pyi"):
                z.write(p, p.relative_to(root / "python_modules").as_posix())
                count += 1
print(f"{count} files in {out.name}, {out.stat().st_size // 1024} KiB")
