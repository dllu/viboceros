"""Freeze direct offset bases used to investigate creased Inset cusp fans."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_fan_fit_probe import cases

if __name__=='__main__':
    query=request();query['operations']=cases()
    capture('inset_fan_fit',query,['generate_inset_fan_fit.py','generate_inset_surface.py',
            'inset_fan_fit_probe.py','inset_continuity_probe.py','inset_multiedge_probe.py',
            'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetFanFit20261103',timeout=1200)
