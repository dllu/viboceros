"""Freeze independent public plane and planar cap constructions."""
from .capture_family import capture
from .apex_splitting_cases import planes
if __name__=='__main__':
    capture('apex_splitting_planes',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=planes()),
        ['generate_apex_splitting_planes.py','apex_splitting_cases.py','extrude_apex_subd_cases.py','apex_plane_sdk_probe.py','revolve_subd_cases.py','revolve_splitting_cases.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleApexSplittingPlanes20261106',timeout=1200)
