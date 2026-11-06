"""Freeze planar-region nesting and independent-overlap command evidence."""
from .capture_family import capture
from .planar_regions_cases import pilot
if __name__=='__main__':
    capture('planar_regions_pilot',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=pilot()),
        ['generate_planar_regions_pilot.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarRegionsPilot20261106',timeout=1200)
