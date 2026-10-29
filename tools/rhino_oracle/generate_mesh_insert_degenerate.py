"""Capture insertion on independently authored flat-corner quads and neighbors."""
from .capture_family import capture
from .mesh_insert_inputs import save_sources
from .generate_insert_point import SOURCES
from .mesh_insert_degenerate_cases import CASES
from .insert_point_cases import request as base

def request():
    q=base(full=True);q['operations']=[dict(op='mesh_insert_degenerate',id=k,case=k)for k in CASES];return q
if __name__=='__main__':
    capture('mesh_insert_degenerate',request(),['generate_mesh_insert_degenerate.py','mesh_insert_degenerate_cases.py',
        'mesh_insert_degenerate_probe.py','mesh_insert_inputs.py',*SOURCES], 'VibocerosOracleMeshInsertFlatCorner20261029',timeout=600)

    save_sources('mesh_insert_degenerate')
