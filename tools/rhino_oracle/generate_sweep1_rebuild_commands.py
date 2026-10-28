"""Scripted Sweep1 rebuild options, independently retained from SDK captures."""
from .capture_family import capture
from .generate_sweep1_rebuild import request


if __name__=='__main__':
    query=request(command=True)
    query['operations']=[op for op in query['operations'] if op['rebuild_point_count'] in [0,3,5,10]]
    capture('sweep1_rebuild_commands',query,[
        'generate_sweep1_rebuild_commands.py','generate_sweep1_rebuild.py','sweep1_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRebuildCommand20261027',timeout=240)
