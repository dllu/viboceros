"""Freeze Inset tolerance refinement separately from source construction."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_fan_basis_probe import refinement

if __name__=='__main__':
    query=request();query['operations']=refinement()
    capture('inset_fan_refinement',query,['generate_inset_fan_refinement.py','generate_inset_surface.py',
            'inset_fan_basis_probe.py','inset_continuity_probe.py','inset_multiedge_probe.py',
            'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetFanRefinement20261103',timeout=1200)
