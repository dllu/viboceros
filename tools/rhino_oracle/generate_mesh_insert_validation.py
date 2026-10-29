"""Record public validity diagnostics; inputs come from captured outputs."""
from .capture_family import capture
from .mesh_insert_validation_probe import NAMES
from .generate_merge_faces_command import request as base

def request():
    q=base();q['operations']=[dict(op='mesh_insert_validation',id=k,case=k)for k in NAMES];return q
if __name__=='__main__':
    capture('mesh_insert_validation',request(),['generate_mesh_insert_validation.py','mesh_insert_validation_probe.py',
        'fixtures/mesh_insert_validation_assets.json','join_probe.py','merge_edges_probe.py','client.py','structural_sdk.py','rhino_worker.py'],
        'VibocerosOracleMeshInsertValidity20261029',timeout=300)
