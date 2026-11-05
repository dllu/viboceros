"""Freeze native straight-extrusion storage and independent split policies."""
from .capture_family import capture
from .extrusion_splitting_cases import curved
if __name__=='__main__':
    capture('extrusion_splitting_curved',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=curved()),
        ['generate_extrusion_splitting_curved.py','extrusion_splitting_cases.py','extrude_apex_subd_cases.py','extrusion_splitting_probe.py','revolve_subd_cases.py','revolve_splitting_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleExtrusionSplittingCurved20261106',timeout=1200)
