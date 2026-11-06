"""Confirm crossing holdout regions with translated native area integration."""
from .capture_family import capture
from .planar_crossings_cases import holdouts
if __name__=='__main__':
    capture('planar_crossings_confirmations',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=holdouts()),
        ['generate_planar_crossings_confirmations.py','planar_crossings_cases.py','planar_crossings_probe.py','planar_regions_cases.py','planar_regions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOraclePlanarCrossingsConfirmations20261106',timeout=1200)
