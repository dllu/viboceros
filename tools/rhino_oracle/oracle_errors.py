"""Public exceptions shared by oracle transport, preflight and input helpers."""

class OracleError(RuntimeError):
    """Base error raised by the host-side oracle driver."""


class OracleProtocolError(OracleError):
    """Raised when an engine returns an invalid protocol response."""
