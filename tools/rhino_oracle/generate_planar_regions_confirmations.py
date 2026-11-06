"""Confirm transformed regions with polished, spatially checked loop witnesses."""
from .capture_family import capture
from .planar_regions_cases import transforms
if __name__=='__main__':
    capture('planar_regions_confirmations',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=transforms()),
        ['generate_planar_regions_confirmations.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarRegionsConfirmations20261106',timeout=1200)
