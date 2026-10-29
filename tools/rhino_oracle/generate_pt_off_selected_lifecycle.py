"""Postselection preserves unrelated owners/grips across acceptance and Cancel."""
from .capture_family import capture


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='pt_off_selected',id='curve-'+mode+'-'+pattern,
                                 kind='curve',mode=mode,pattern=pattern)
                            for mode in ['control','edit']
                            for pattern in ['postselect-peer','postselect-grip','postcancel-peer','postcancel-grip','postcancel']])


if __name__=='__main__':
    capture('pt_off_selected_lifecycle',request(),[
        'generate_pt_off_selected_lifecycle.py','pt_off_selected_probe.py','hide_pt_input.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOraclePtOffSelectedLifecycle20261029',timeout=300)
