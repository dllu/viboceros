"""Capture the original six Shell recipes in a private licensed Rhino session."""
from .capture_family import capture
from .shell_probe import request


if __name__ == '__main__':
    capture('shell_sdk_baseline', request(), [
        'generate_shell_sdk_baseline.py', 'shell_probe.py', 'client.py',
        'rhino_worker.py', 'structural_sdk.py',
    ], 'VibocerosOracleShellSdkBaseline20261108', timeout=1200)
