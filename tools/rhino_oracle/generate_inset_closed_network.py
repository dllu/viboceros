"""Freeze source-edge subdivision through an adjacent unselected face."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_closed_network_probe import cases

if __name__=='__main__':
    query=request();query['operations']=cases()
    capture('inset_closed_network',query,['generate_inset_closed_network.py','generate_inset_surface.py',
            'inset_closed_network_probe.py','inset_fan_basis_probe.py','inset_continuity_probe.py',
            'inset_multiedge_probe.py','inset_outputs_probe.py','inset_planar_curves.py',
            'inset_surface_probe.py','inset_surface_command.py','join_probe.py','merge_edges_probe.py',
            'client.py','rhino_worker.py','structural_sdk.py'],
            'VibocerosOracleInsetClosedNetwork20261103',timeout=1200)
