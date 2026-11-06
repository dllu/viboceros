"""Capture material membership and native mesh coverage of crossing trims."""
from .capture_family import capture
from .planar_crossings_cases import details
if __name__=='__main__':
    capture('planar_crossings_details',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=details()),
        ['generate_planar_crossings_details.py','planar_crossings_cases.py','planar_crossings_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarCrossingsDetails20261106',timeout=1200)
