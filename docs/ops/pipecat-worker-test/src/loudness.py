"""Stand-in for the native `loudness` package (no Pyodide build). Audio only: every use fails."""


def __getattr__(name):
    raise RuntimeError(f"loudness.{name}: no audio in this Worker")
