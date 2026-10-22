"""Fixed public topology-cache and stored color-transparency Weld experiments."""
from copy import deepcopy
try:
    from .mesh_weld_channels_cases import CASES as BASE, FOLLOWUP
    from .mesh_weld_channels_stress_cases import CASES as STRESS
except (ImportError, ValueError):
    from mesh_weld_channels_cases import CASES as BASE, FOLLOWUP
    from mesh_weld_channels_stress_cases import CASES as STRESS

CASES={}
RADIAL_TWINS={}
for name in ['cube_all','cube_rotated','cube_reverse_raw','cube_triangles',
             'holdout_grid_quads_2','holdout_grid_triangles_2',
             'holdout_cube_raw_5_0','holdout_cube_rotation_2']:
    case=deepcopy(BASE[name] if name in BASE else STRESS[name])
    case['sort_radial']=True
    key='radial_'+name
    CASES[key]=case
    RADIAL_TWINS[key]=name

for name in ['pair_flat','half_shared','cube_all']:
    for profile in ['opaque','transparent','ramp','extremes']:
        case=deepcopy(BASE[name] if name in BASE else FOLLOWUP[name])
        count=len(case['source']['vertices'])
        values={'opaque':[0]*count,'transparent':[255]*count,
                'ramp':[(17*i)%256 for i in range(count)],
                'extremes':[[0,255,1,254,127,128,63][i%7] for i in range(count)]}
        case['transparency']=values[profile]
        CASES['transparency_'+name+'_'+profile]=case


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_weld_channels',id=name,case=name) for name in CASES])
