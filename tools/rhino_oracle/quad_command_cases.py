"""Independent scripted QuadrangulateMesh workflows, not SDK conversion calls."""
from copy import deepcopy
SQUARE = [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.]]
WARP = [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,1.]]
TRIANGLES = [[0,1,2],[0,2,3]]


def scene(vertices, arguments=''):
    return dict(command='QuadrangulateMesh', whole=True, preselected=True,
                sources=[dict(kind='mesh',vertices=deepcopy(vertices),faces=deepcopy(TRIANGLES))],
                command_arguments=arguments)


DISCOVERY = {
    'quad_command_default_square': scene(SQUARE),
    'quad_command_default_warp': scene(WARP),
    'quad_command_explicit_warp': scene(WARP, '_Planarity=90 _Rectangularity=2'),
}
SHALLOW = [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,.04]]
SKEW = [[0.,0.,0.],[3.,0.,0.],[1.,1.,0.],[0.,1.,0.]]
MAIN = {
    'quad_command_fresh_defaults': scene(SHALLOW),
    'quad_command_low_angle': scene(SHALLOW, '_Planarity=1 _Rectangularity=2'),
    'quad_command_remembered_low': scene(SHALLOW),
    'quad_command_standard_angle': scene(SHALLOW, '_Planarity=5 _Rectangularity=2'),
    'quad_command_remembered_standard': scene(SHALLOW),
    'quad_command_strict_ratio': scene(SKEW, '_Planarity=5 _Rectangularity=1'),
    'quad_command_remembered_strict': scene(SKEW),
    'quad_command_relaxed_ratio': scene(SKEW, '_Planarity=5 _Rectangularity=3'),
    'quad_command_remembered_relaxed': scene(SKEW),
    'quad_command_cancel_options': dict(scene(SHALLOW, '_Planarity=1 _Rectangularity=2'), cancel=True),
    'quad_command_after_cancel': scene(SHALLOW),
    'quad_command_first_square': dict(scene(SQUARE, '_All _Enter _Planarity=5 _Rectangularity=2'), whole=False, preselected=False),
    'quad_command_first_warp': dict(scene(WARP, '_All _Enter _Planarity=5 _Rectangularity=2'), whole=False, preselected=False),
    'quad_command_channels_ngon': scene(SQUARE, '_Planarity=5 _Rectangularity=2'),
    'quad_command_mixed': scene(SQUARE, '_Planarity=5 _Rectangularity=2'),
    'quad_command_raw_seam': scene(SQUARE, '_Planarity=5 _Rectangularity=2'),
    'quad_command_cancel_selection': dict(scene(SQUARE), whole=False, preselected=False, cancel=True),
}
MAIN['quad_command_channels_ngon']['sources'][0].update(
    colors=[[255,0,0,255],[0,255,0,128],[0,0,255,0],[90,80,70,255]],
    texture_coordinates=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],
    normals=[[1.,0.,0.]]*4, ngons=[dict(vertices=[0,1,2,3],faces=[0,1])])
MAIN['quad_command_channels_ngon']['mesh_channels']=True
MAIN['quad_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+3,y,z]for x,y,z in SQUARE],faces=[[0,1,2,3]]))
MAIN['quad_command_raw_seam']['sources'][0].update(vertices=SQUARE[:3]+[SQUARE[0],SQUARE[2],SQUARE[3]],faces=[[0,1,2],[3,4,5]])
CASES = dict(DISCOVERY, **MAIN)


def request(main=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='quadrangulate_mesh_command',id=k,case=k)for k in (MAIN if main else DISCOVERY)])


def validate(op):
    try:
        valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='quadrangulate_mesh_command'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed QuadrangulateMesh workflow required')
