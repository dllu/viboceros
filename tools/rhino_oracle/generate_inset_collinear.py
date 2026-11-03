"""Freeze connector mapping along subdivided straight boundary sections."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_collinear_probe import cases

if __name__=='__main__':
    query=request();query['operations']=cases()
    capture('inset_collinear_sdk',query,['generate_inset_collinear.py','generate_inset_surface.py',
            'inset_collinear_probe.py','inset_continuity_probe.py','inset_multiedge_probe.py',
            'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetCollinear20261103',timeout=900)
