"""Alternate changed/no-op commands with independent per-case Undo/Redo."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as base
from .generate_mesh_face_collapse_holdouts import request as holdouts


def request():
    query = base()
    by_id = {op['id']:op for op in base()['operations']+holdouts()['operations']}
    query['operations'] = [dict(by_id[name],op='mesh_face_collapse_mode',display_mode='Shaded')
                           for name in ['closed-cube-none','single-triangle-none','fan-none','single-quad-none']]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_history', request(), [
        'generate_mesh_face_collapse_history.py','generate_mesh_face_collapse.py',
        'generate_mesh_face_collapse_holdouts.py','mesh_face_collapse_probe.py',
        'conic_getter_input.py','managed_mouse_callback.py','snap_environment.py',
        'number_token.py','catenary_point_picker.py','group_picking.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseHistory20261029', timeout=240)
