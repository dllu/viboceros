"""Public conversion diagnostics, using frozen output-derived polygon records."""
from .capture_family import capture
from .generate_mesh_insert_validation import request as base

def request():
    q=base()
    for op in q['operations']:op['op']='mesh_insert_conversion'
    return q
if __name__=='__main__':
    capture('mesh_insert_conversion',request(),['generate_mesh_insert_conversion.py','generate_mesh_insert_validation.py','mesh_insert_validation_probe.py',
        'fixtures/mesh_insert_validation_assets.json','join_probe.py','merge_edges_probe.py','client.py','structural_sdk.py','rhino_worker.py'],
        'VibocerosOracleMeshInsertConversion20261029',timeout=300)
