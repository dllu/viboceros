"""Retain both public CreateShell distance signs without assuming validity."""
from .capture_family import capture
from .shell_probe import request


if __name__ == '__main__':
    query = request()
    for op in query['operations']:
        op['diagnostic'] = True
    capture('shell_sdk_signed', query, [
        'generate_shell_sdk_signed.py', 'shell_probe.py', 'client.py',
        'rhino_worker.py', 'structural_sdk.py',
    ], 'VibocerosOracleShellSdkSigned20261108', timeout=1200)
