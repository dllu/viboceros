"""Compare Rhino's float and double mesh positions after a face collapse."""
from .capture_family import capture
from .generate_mesh_face_collapse import request as original


def request():
    query = original()
    query['operations'] = [dict(op,op='mesh_face_collapse_precision',
                                id=op['id']+'-'+precision,precision=precision)
                           for op in query['operations'] if op['channels']=='none'
                           for precision in ['single','double']]
    return query


if __name__ == '__main__':
    capture('mesh_face_collapse_precision', request(), [
        'generate_mesh_face_collapse_precision.py','generate_mesh_face_collapse.py',
        'mesh_face_collapse_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapsePrecision20261029', timeout=240)
