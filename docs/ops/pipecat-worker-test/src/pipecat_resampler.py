"""Stand-in for the native `pipecat-resampler` package (no Pyodide build). Audio only."""


class _NoAudio:
    def __init__(self, *args, **kwargs):
        raise RuntimeError("pipecat_resampler: no audio in this Worker")


class Resampler(_NoAudio):
    pass


class StreamResampler(_NoAudio):
    pass
