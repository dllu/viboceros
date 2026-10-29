"""Exercise every face index, avoiding an index-zero-only reference."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    query['operations'] = [dict(op, id=op['id']+'-face-'+str(face), face=face)
                           for op in query['operations'] if op['channels']=='none'
                           for face in range(len(op['faces']))]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_indices', request(), [
        'generate_mesh_face_collapse_indices.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseIndices20261029', timeout=240)
