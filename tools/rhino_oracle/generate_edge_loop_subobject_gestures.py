"""Freeze Ctrl+Shift gestures with unique edge targets and ordinary target mode."""
from .capture_family import capture
from .edge_loop_gesture_cases import subobject_cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=subobject_cases())
    capture('edge_loop_subobject_gestures',query,['generate_edge_loop_subobject_gestures.py','edge_loop_gestures.py','edge_loop_gesture_cases.py','edge_loop_idle_worker.py',
        'edge_loop_probe.py','edge_loop_cases.py','subd_join_probe.py','subd_join_cases.py',
        'client.py','component_picking.py','structural_sdk.py'],
        'VibocerosOracleEdgeLoopSubobjectGestures20261105',timeout=600)
