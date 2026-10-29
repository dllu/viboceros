"""Observe component-modifier face picks with no manual input."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    query['operations'] = [dict(op,op='mesh_face_collapse_click')
                           for op in query['operations'] if op['id'] in ['fan-none','quad-none']]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_component_pick', request(), [
        'generate_mesh_face_collapse_component_pick.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','mesh_face_collapse_picker.py','group_picking.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseComponentPick20261029', timeout=180)
