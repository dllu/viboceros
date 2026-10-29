"""Qualify independent repeated-edge paths on planar, warped and reordered sources."""
from .capture_family import capture
from .mesh_insert_inputs import save_sources
from .generate_insert_point import SOURCES
from .mesh_insert_paths_cases import PATH_CASES
from .insert_point_cases import request as base

def request():
    q=base(full=True);q['operations']=[dict(op='mesh_insert_paths',id=k,case=k)for k in PATH_CASES]
    return q

if __name__=='__main__':
    capture('mesh_insert_paths',request(),['generate_mesh_insert_paths.py','mesh_insert_paths_cases.py',
        'mesh_insert_paths_probe.py','mesh_insert_inputs.py',*SOURCES], 'VibocerosOracleMeshInsertPaths20261029',timeout=1200)

    save_sources('mesh_insert_paths')
