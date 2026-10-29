"""Shared bounds for dedicated, private, single-family reference captures."""
from .oracle_errors import OracleProtocolError


def require_dedicated_capture(client, request, family, message, *, strict_iterations=True):
    operations = request['operations']
    if (client.settings_scheme is None
            or (strict_iterations and type(request.get('iterations', 1)) is not int)
            or request.get('iterations', 1) != 1
            or not 1 <= len(operations) <= 64
            or any(op.get('op') != family for op in operations)):
        raise OracleProtocolError(message)
    return operations
