"""Capture each SelChain source group in a fresh private Rhino session."""
import sys
from .capture_family import capture
from .chain_selection_cases import cases
GROUPS=('lines','reversed','corner','cycle','surface','mixed','prefer_curve','branch','tolerance','lifecycle')
def grouped(group):
    rows=cases()
    if group in GROUPS[:8]:return [r for r in rows if r['id'].startswith(group+'_') and r['options'].get('continuity')]
    return rows[72:79] if group=='tolerance' else rows[79:]
if __name__=='__main__':
    group=sys.argv[1]
    if group not in GROUPS:raise ValueError('fixed SelChain capture group required')
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=grouped(group))
    capture('chain_selection_'+group,request,['generate_chain_selection_controls.py','chain_selection_probe.py','chain_selection_cases.py','chain_selection_input.py','chain_selection_driver.py','shrink_face_input.py','component_picking.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleChainSelection'+group.title().replace('_','')+'20261104',timeout=240)
