"""Require prompt, movement and mouse-down acknowledgements for actual picks."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    query['operations'] = [dict(op,op='mesh_face_collapse_getter')
                           for op in query['operations'] if op['id'].startswith(('fan-','quad-'))]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_getter', request(), [
        'generate_mesh_face_collapse_getter.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','conic_getter_input.py','managed_mouse_callback.py',
        'snap_environment.py','number_token.py','catenary_point_picker.py','group_picking.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseGetter20261029', timeout=240)
