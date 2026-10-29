"""Actual getter rejection, continuation and local Undo across disconnected islands."""
from .capture_family import capture
from .mesh_insert_inputs import save_sources
from .generate_insert_point import SOURCES
from .mesh_insert_paths_cases import DISCONNECTED
from .insert_point_cases import request as base

def request():
    q=base(full=True);q['operations']=[dict(op='mesh_insert_paths',id=k,case=k)for k in DISCONNECTED];return q
if __name__=='__main__':
    capture('mesh_insert_disconnected',request(),['generate_mesh_insert_disconnected.py','mesh_insert_paths_cases.py',
        'mesh_insert_paths_probe.py','mesh_insert_inputs.py',*SOURCES], 'VibocerosOracleMeshInsertIslands20261029',timeout=600)

    save_sources('mesh_insert_disconnected')
