"""Independent inputs for public quad triangulation and command workflows."""
from copy import deepcopy

SQUARE = [[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]
QUAD = [[0,1,2,3]]


def mesh(vertices,faces=QUAD,**extra):
    result=dict(vertices=deepcopy(vertices),faces=deepcopy(faces))
    result.update(extra)
    return result


SDK = {
    'triangulate_square':mesh(SQUARE),
    'triangulate_short_bd':mesh([[0.,0.,0.],[1.,0.,0.],[2.,2.,0.],[0.,1.,0.]]),
    'triangulate_warp':mesh([[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]]),
    'triangulate_concave':mesh([[0.,0.,0.],[3.,0.,0.],[.2,.1,0.],[0.,2.,0.]]),
    'triangulate_reversed':mesh(SQUARE,[[3,2,1,0]]),
    'triangulate_cyclic':mesh(SQUARE,[[1,2,3,0]]),
    'triangulate_spatial_tie':mesh([[0.,0.,0.],[1.,0.,1.],[1.,1.,0.],[0.,1.,1.]]),
    'triangulate_triangles':mesh(SQUARE,[[0,1,2],[0,2,3]]),
    'triangulate_unused':mesh(SQUARE+[[99.,99.,99.]]),
    'triangulate_tiny':mesh([[x*1e-8,y*1e-8,z]for x,y,z in SQUARE]),
    'triangulate_translated':mesh([[x+1e7,y-1e7,z+1e7]for x,y,z in SQUARE]),
    'triangulate_skinny':mesh([[0.,0.,0.],[1.,0.,0.],[1.,1e-8,0.],[0.,1.,0.]]),
    'triangulate_channels':mesh(SQUARE,colors=[[255,0,0,0],[0,255,0,127],[0,0,255,255],[90,80,70,0]],uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[1.,0.,0.]]*4),
    'triangulate_ngon':mesh(SQUARE,ngons=[dict(vertices=[0,1,2,3],faces=[0])]),
    'triangulate_two_ngons':mesh(SQUARE+[[x+3,y,z]for x,y,z in SQUARE],[[0,1,2,3],[4,5,6,7]],ngons=[dict(vertices=[4,5,6,7],faces=[1]),dict(vertices=[0,1,2,3],faces=[0])]),
    'triangulate_mixed_slots':mesh(SQUARE+[[x+3,y,z]for x,y,z in SQUARE]+[[8.,0.,0.],[9.,0.,0.],[8.,1.,0.]],[[8,9,10],[0,1,2,3],[4,5,6,7]]),
    'triangulate_ngon_two_quads':mesh([[0.,0.,0.],[1.,0.,0.],[2.,0.,0.],[0.,1.,0.],[1.,1.,0.],[2.,1.,0.]],[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[1,0])]),
}


def scene(name,**extra):
    source=deepcopy(SDK['triangulate_'+name])
    if 'uv'in source:source['texture_coordinates']=source.pop('uv')
    if 'colors'in source:
        source['colors']=[c[:3]+[255-c[3]]for c in source['colors']]
    source['kind']='mesh'
    result=dict(command='TriangulateMesh',whole=True,preselected=True,sources=[source],mesh_channels=True)
    result.update(extra)
    return result


COMMANDS = {'triangulate_command_'+name:scene(name)for name in ['square','short_bd','warp','triangles','channels','ngon','two_ngons','mixed_slots','ngon_two_quads','tiny']}
COMMANDS['triangulate_command_mixed']=scene('square')
COMMANDS['triangulate_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=deepcopy(QUAD)))
COMMANDS['triangulate_command_two_meshes']=scene('square')
COMMANDS['triangulate_command_two_meshes']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=deepcopy(QUAD)))
COMMANDS['triangulate_command_postselect']=scene('square',whole=False,preselected=False,command_arguments='_All _Enter')
COMMANDS['triangulate_command_cancel']=scene('square',whole=False,preselected=False,cancel=True)

HOLDOUTS = {}
HOLDOUTS['triangulate_holdout_triangle_ngon']=scene('triangles')
HOLDOUTS['triangulate_holdout_triangle_ngon']['sources'][0]['ngons']=[dict(vertices=[0,1,2,3],faces=[0,1])]
for name,reverse in [('partial_ngons',False),('reversed_ngons',True)]:
    case=scene('square');source=case['sources'][0]
    source['vertices'] += [[4.,0.,0.],[5.,0.,0.],[4.,1.,0.]]
    source['faces'] += [[4,5,6]]
    source['ngons']=[dict(vertices=[0,1,2,3],faces=[0]),dict(vertices=[4,5,6],faces=[1])]
    if reverse:source['ngons'].reverse()
    HOLDOUTS['triangulate_holdout_'+name]=case
HOLDOUTS['triangulate_holdout_cancel_picks']=scene('square',whole=False,preselected=False,command_arguments='_All',cancel=True)
HOLDOUTS['triangulate_holdout_postselect_triangles']=scene('triangles',whole=False,preselected=False,command_arguments='_All _Enter')


def request(commands=False,holdouts=False):
    if commands and holdouts:raise ValueError('one triangulation family required')
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='triangulate_mesh_command'if commands or holdouts else'triangulate_mesh_sdk',id=k,case=k)for k in(HOLDOUTS if holdouts else COMMANDS if commands else SDK)])


def validate(op):
    try:
        valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['id']==op['case']and((op['op']=='triangulate_mesh_sdk'and op['case']in SDK)or(op['op']=='triangulate_mesh_command'and(op['case']in COMMANDS or op['case']in HOLDOUTS)))
    except TypeError:valid=False
    if not valid:raise ValueError('fixed triangulation recipe required')
