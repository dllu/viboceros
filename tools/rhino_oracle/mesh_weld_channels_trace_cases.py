"""Public precompaction diagnostics paired with independently valid Mesh.Weld twins."""
from copy import deepcopy
try:
    from .mesh_weld_channels_cases import CASES as BASE
    from .mesh_weld_channels_stress_cases import CASES as STRESS
except (ImportError, ValueError):
    from mesh_weld_channels_cases import CASES as BASE
    from mesh_weld_channels_stress_cases import CASES as STRESS
CASES={}
TWINS={}
for name in ['cube_all','cube_rotated','cube_reverse_raw','cube_triangles',
             'holdout_grid_quads_2','holdout_grid_triangles_2',
             'holdout_cube_raw_5_0','holdout_cube_rotation_2']:
    source=deepcopy(BASE[name] if name in BASE else STRESS[name])
    source['late_invalid_face']=True
    key='trace_'+name
    CASES[key]=source
    TWINS[key]=name


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_weld_channels',id=name,case=name) for name in CASES])
