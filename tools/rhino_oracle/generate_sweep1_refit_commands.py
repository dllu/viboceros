"""Actual Sweep1 section-refit command outputs, separate from the SDK family."""
from .capture_family import capture
from .generate_sweep1_refit import request


def query():
    root=request(command=True)
    root['operations']=[op for op in root['operations'] if op['refit_tolerance']>=.001]
    return root


if __name__=='__main__':
    capture('sweep1_refit_commands',query(),[
        'generate_sweep1_refit_commands.py','generate_sweep1_refit.py',
        'generate_sweep1_rebuild.py','sweep1_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRefitCommand20261027',timeout=300)
