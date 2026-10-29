"""Actual shaded-viewport picks with prompt and mouse acknowledgements."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    query['operations'] = [dict(op,op='mesh_face_collapse_shaded')
                           for op in query['operations'] if op['id'].startswith(('fan-','quad-'))]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_shaded', request(), [
        'generate_mesh_face_collapse_shaded.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','conic_getter_input.py','managed_mouse_callback.py',
        'snap_environment.py','number_token.py','catenary_point_picker.py','group_picking.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseShaded20261029', timeout=240)
