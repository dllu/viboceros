"""Freeze raw curve and split-surface output modes for crossed Inset fans."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_fan_commands_probe import cases

if __name__=='__main__':
    query=request();query['operations']=cases()
    capture('inset_fan_commands',query,['generate_inset_fan_commands.py','generate_inset_surface.py',
            'inset_fan_commands_probe.py','inset_fan_interior_probe.py','inset_fan_basis_probe.py',
            'inset_continuity_probe.py','inset_multiedge_probe.py','inset_outputs_probe.py',
            'inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetFanCommands20261103',timeout=1200)
