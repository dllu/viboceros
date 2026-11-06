"""Confirm PlanarSrf recovered networks with dangling and rational branches."""
from .capture_family import capture
from .planar_chains_cases import network_holdouts
if __name__=='__main__':
    capture('planar_chain_network_holdouts',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=network_holdouts()),
        ['generate_planar_chain_network_holdouts.py','planar_chains_cases.py','planar_chains_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarChainNetworkHoldouts20261106',timeout=1500)
