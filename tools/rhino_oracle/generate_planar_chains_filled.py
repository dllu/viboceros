"""Check how partially closed chains interact with complete planar regions."""
from .capture_family import capture
from .planar_chains_cases import filled
if __name__=='__main__':
    capture('planar_chains_filled',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=filled()),
        ['generate_planar_chains_filled.py','planar_chains_cases.py','planar_chains_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarChainsFilled20261106',timeout=1200)
