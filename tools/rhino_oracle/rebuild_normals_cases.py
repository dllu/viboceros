"""Independent normal rebuilding inputs, with raw seams and conflicting winding."""
from copy import deepcopy

SQUARE=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]
FOLD=[[0.,0.,0.],[2.,0.,0.],[0.,1.,0.],[0.,0.,3.]]


def mesh(vertices,faces,**extra):
    result=dict(vertices=deepcopy(vertices),faces=deepcopy(faces));result.update(extra);return result


SDK={
    'rebuild_normals_triangle':mesh(SQUARE[:3],[[0,1,2]]),
    'rebuild_normals_quad':mesh(SQUARE,[[0,1,2,3]]),
    'rebuild_normals_zero':mesh(SQUARE,[[0,1,2,3]],normals=[[0.,0.,0.]]*4),
    'rebuild_normals_custom':mesh(SQUARE,[[0,1,2,3]],normals=[[1.,0.,0.]]*4),
    'rebuild_normals_warp':mesh([[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]],[[0,1,2,3]],normals=[[0.,1.,0.]]*4),
    'rebuild_normals_fold':mesh(FOLD,[[0,1,2],[1,0,3]]),
    'rebuild_normals_seam':mesh(FOLD[:3]+[FOLD[1],FOLD[0],FOLD[3]],[[0,1,2],[3,4,5]]),
    'rebuild_normals_opposed':mesh(SQUARE[:3],[[0,1,2],[2,1,0]]),
    'rebuild_normals_unused':mesh(SQUARE+[[99.,99.,99.]],[[0,1,2,3]],normals=[[1.,0.,0.]]*5),
    'rebuild_normals_tiny':mesh([[x*1e-8,y*1e-8,z]for x,y,z in SQUARE],[[0,1,2,3]]),
    'rebuild_normals_channels':mesh(SQUARE,[[0,1,2,3]],colors=[[255,0,0,0],[0,255,0,127],[0,0,255,255],[90,80,70,0]],uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[1.,0.,0.]]*4),
    'rebuild_normals_ngon':mesh(SQUARE,[[0,1,2],[0,2,3]],ngons=[dict(vertices=[0,1,2,3],faces=[1,0])],normals=[[1.,0.,0.]]*4),
    'rebuild_normals_nonmanifold':mesh(FOLD+[[0.,-1.,0.]],[[0,1,2],[1,0,3],[0,1,4]]),
    'rebuild_normals_collapsed':mesh([[0.,0.,0.],[1.,0.,0.],[2.,0.,0.],[0.,1.,0.]],[[0,1,2],[0,1,3]]),
    'rebuild_normals_reversed':mesh(SQUARE,[[3,2,1,0]]),
}


def scene(name,**extra):
    key='rebuild_normals_'+name
    source=deepcopy(SDK[key]if key in SDK else EXTRA_SDK[key]);source['kind']='mesh'
    if 'uv'in source:source['texture_coordinates']=source.pop('uv')
    if 'colors'in source:source['colors']=[c[:3]+[255-c[3]]for c in source['colors']]
    if 'normals'not in source:source['clear_normals']=True
    result=dict(command='RebuildMeshNormals',whole=True,preselected=True,sources=[source],mesh_channels=True)
    result.update(extra);return result


COMMANDS={'rebuild_normals_command_'+name:scene(name)for name in ['triangle','quad','custom','warp','fold','seam','opposed','unused','tiny','channels','ngon','nonmanifold','reversed']}
COMMANDS['rebuild_normals_command_mixed']=scene('custom')
COMMANDS['rebuild_normals_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2,3]]))
COMMANDS['rebuild_normals_command_two']=scene('custom')
COMMANDS['rebuild_normals_command_two']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2,3]],normals=[[0.,1.,0.]]*4))
COMMANDS['rebuild_normals_command_postselect']=scene('custom',whole=False,preselected=False,command_arguments='_All _Enter')
COMMANDS['rebuild_normals_command_cancel']=scene('custom',whole=False,preselected=False,cancel=True)
COMMANDS['rebuild_normals_command_cancel_picks']=scene('custom',whole=False,preselected=False,command_arguments='_All',cancel=True)

EXTRA_SDK={}
EXTRA_SDK['rebuild_normals_holdout_channels']=mesh([[99.,99.,99.],SQUARE[0],SQUARE[1],[88.,88.,88.],SQUARE[2],SQUARE[3],[77.,77.,77.]],[[1,2,4,5]],colors=[[i*20,255-i*20,i*15,i*10]for i in range(7)],uv=[[float(i),float(i*i)]for i in range(7)],normals=[[1.,0.,0.]]*7,ngons=[dict(vertices=[1,2,4,5],faces=[0])])
seam=SDK['rebuild_normals_seam']
EXTRA_SDK['rebuild_normals_holdout_seams']=mesh([[99.,99.,99.]]+seam['vertices']+[[88.,88.,88.]],[[i+1 for i in f]for f in seam['faces']],uv=[[float(i),float(i%3)]for i in range(8)])
EXTRA_SDK['rebuild_normals_holdout_ngons']=mesh([[99.,99.,99.]]+SQUARE+[[x+4,y,z]for x,y,z in SQUARE],[[1,2,3,4],[5,6,7,8]],ngons=[dict(vertices=[5,6,7,8],faces=[1]),dict(vertices=[1,2,3,4],faces=[0])])
EXTRA_SDK['rebuild_normals_holdout_nonunit']=mesh(SQUARE,[[0,1,2,3]],normals=[[5.,2.,1.]]*4)
EXTRA_COMMANDS={'rebuild_normals_holdout_command_'+name:scene('holdout_'+name)for name in ['channels','seams','ngons']}
EXTRA_COMMANDS['rebuild_normals_holdout_command_noop']=scene('quad')
EXTRA_COMMANDS['rebuild_normals_holdout_command_noop']['sources'][0].pop('clear_normals')
EXTRA_COMMANDS['rebuild_normals_holdout_command_noop']['sources'][0]['normals']=[[0.,0.,1.]]*4


def request(commands=False,holdouts=False):
    if commands and holdouts:raise ValueError('one normal rebuild family required')
    cases=list(EXTRA_SDK)+list(EXTRA_COMMANDS)if holdouts else COMMANDS if commands else SDK
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='rebuild_mesh_normals_command'if commands or k in EXTRA_COMMANDS else'rebuild_mesh_normals_sdk',id=k,case=k)for k in cases])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['id']==op['case']and((op['op']=='rebuild_mesh_normals_sdk'and(op['case']in SDK or op['case']in EXTRA_SDK))or(op['op']=='rebuild_mesh_normals_command'and(op['case']in COMMANDS or op['case']in EXTRA_COMMANDS)))
    except TypeError:valid=False
    if not valid:raise ValueError('fixed normal rebuild recipe required')
