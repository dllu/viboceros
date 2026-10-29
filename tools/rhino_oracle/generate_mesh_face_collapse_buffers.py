"""Read actual float storage from a CRC-guarded owned copy without double vertices."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as base


def request():
    query = base()
    query['operations'] = [dict(op,op='mesh_face_collapse_buffers',id=op['id']+'-'+precision,precision=precision)
                           for op in query['operations'] if op['id'].split('-')[0] in ['fan','quad','tetra','nonplanar']
                           for precision in ['single','double']]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_buffers', request(), [
        'generate_mesh_face_collapse_buffers.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseBuffers20261029', timeout=300)
