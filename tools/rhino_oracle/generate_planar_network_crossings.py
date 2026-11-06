"""Check open self-crossing recovery and asymmetric joins in filled networks."""
from .capture_family import capture
from .planar_chains_cases import network_crossings
if __name__=='__main__':
    capture('planar_network_crossings',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=network_crossings()),
        ['generate_planar_network_crossings.py','planar_chains_cases.py','planar_chains_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarNetworkCrossings20261106',timeout=1500)
