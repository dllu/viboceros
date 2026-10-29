"""Compare selection-only, PointsOff and PtOffSelected Redo outcomes."""
from .capture_family import capture


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='pt_off_selected_history',id='curve-control-'+pattern+'-'+action,
                                 kind='curve',mode='control',pattern=pattern,control_action=action)
                            for pattern in ['none','owner','grip']
                            for action in ['none','PointsOff','PtOffSelected']])


if __name__=='__main__':
    capture('pt_off_selected_history',request(),[
        'generate_pt_off_selected_history.py','pt_off_selected_probe.py','hide_pt_input.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOraclePtOffSelectedHistory20261029',timeout=300)
