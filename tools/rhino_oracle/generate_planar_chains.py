"""Capture native PlanarSrf joining of open curve boundaries."""
from .capture_family import capture
from .planar_chains_cases import pilot
if __name__=='__main__':
    capture('planar_chains',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=pilot()),
        ['generate_planar_chains.py','planar_chains_cases.py','planar_chains_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarChains20261106',timeout=1200)
