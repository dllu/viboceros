"""Compare documented point input with component preselection."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    for op in query['operations']:
        op['op'] = 'mesh_face_collapse_point'
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_point', request(), [
        'generate_mesh_face_collapse_point.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapsePoint20261029', timeout=300)
