"""Explicit Ribbon option assignments; earlier toggle attempts stay archived."""
from .capture_family import capture
from .generate_ribbon import request


if __name__=='__main__':
    capture('ribbon_options',request(),['generate_ribbon_options.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonOptions20261109',timeout=1200)
