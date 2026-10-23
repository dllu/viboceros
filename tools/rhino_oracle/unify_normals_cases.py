"""Independent winding, channel and connected-region normal unification inputs."""
from copy import deepcopy
import math
SQUARE=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]
TETRA=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]
TETRA_FACES=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]]


def mesh(vertices,faces,**extra):
    v=dict(vertices=deepcopy(vertices),faces=deepcopy(faces));v.update(extra);return v


SDK={
    'unify_normals_consistent':mesh(SQUARE,[[0,1,2],[0,2,3]]),
    'unify_normals_pair':mesh(SQUARE,[[0,1,2],[0,3,2]]),
    'unify_normals_pair_reordered':mesh(SQUARE,[[0,3,2],[0,1,2]]),
    'unify_normals_custom':mesh(SQUARE,[[0,1,2],[0,3,2]],normals=[[1.,0.,0.]]*4),
    'unify_normals_channels':mesh(SQUARE,[[0,1,2],[0,3,2]],normals=[[1.,0.,0.]]*4,uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],colors=[[255,0,0,0],[0,255,0,127],[0,0,255,255],[90,80,70,0]]),
    'unify_normals_seam':mesh(SQUARE[:3]+[SQUARE[0],SQUARE[3],SQUARE[2]],[[0,1,2],[3,4,5]],normals=[[1.,0.,0.]]*6),
    'unify_normals_unused':mesh(SQUARE+[[99.,99.,99.]],[[0,1,2],[0,3,2]],normals=[[1.,0.,0.]]*5),
    'unify_normals_quads':mesh([[0.,0.,0.],[1.,0.,0.],[2.,0.,0.],[0.,1.,0.],[1.,1.,0.],[2.,1.,0.]],[[0,1,4,3],[4,5,2,1]]),
    'unify_normals_nonmanifold':mesh(SQUARE+[[1.,.5,1.]],[[0,1,2],[0,3,2],[0,4,2]]),
    'unify_normals_duplicate':mesh(SQUARE[:3],[[0,1,2],[0,1,2]]),
    'unify_normals_opposed':mesh(SQUARE[:3],[[0,1,2],[2,1,0]]),
    'unify_normals_ngon':mesh(SQUARE+[[3.,.5,1.]],[[0,1,2],[0,2,3],[1,2,4]],ngons=[dict(vertices=[0,1,2,3],faces=[0,1])]),
    'unify_normals_ngon_first_outside':mesh(SQUARE+[[3.,.5,1.]],[[1,2,4],[0,1,2],[0,2,3]],ngons=[dict(vertices=[0,1,2,3],faces=[1,2])]),
    'unify_normals_tiny':mesh([[x*1e-8,y*1e-8,z]for x,y,z in SQUARE],[[0,1,2],[0,3,2]]),
    'unify_normals_translated':mesh([[x+1e7+.125,y-1e7+.375,z+1e7+.625]for x,y,z in SQUARE],[[0,1,2],[0,3,2]]),
}
for index in range(4):
    faces=deepcopy(TETRA_FACES);faces[index]=list(reversed(faces[index]));SDK['unify_normals_tetra_'+str(index)]=mesh(TETRA,faces)
SDK['unify_normals_disjoint']=mesh(SQUARE+[[x+5,y,z]for x,y,z in SQUARE],[[0,1,2],[0,3,2],[4,7,6],[4,5,6]])


def scene(name,**extra):
    source=deepcopy(SDK['unify_normals_'+name]);source['kind']='mesh'
    if 'uv'in source:source['texture_coordinates']=source.pop('uv')
    if 'colors'in source:source['colors']=[c[:3]+[255-c[3]]for c in source['colors']]
    v=dict(command='UnifyMeshNormals',whole=True,preselected=True,sources=[source],mesh_channels=True);v.update(extra);return v


DISCOVERY={
    'unify_normals_discovery_pair':scene('channels'),
    'unify_normals_discovery_noop':scene('consistent'),
    'unify_normals_discovery_two':scene('pair'),
    'unify_normals_discovery_postselect':scene('channels',whole=False,preselected=False,command_arguments='_All _Enter'),
}
DISCOVERY['unify_normals_discovery_two']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2],[0,3,2]]))

EXTRA={}
EXTRA['unify_extra_inward']=mesh(TETRA,[list(reversed(f))for f in TETRA_FACES])
for index in [0,3]:
    faces=[list(reversed(f))for f in TETRA_FACES];faces[index]=deepcopy(TETRA_FACES[index]);EXTRA['unify_extra_inward_bad_'+str(index)]=mesh(TETRA,faces,normals=[[1.,0.,0.]]*4)
for a in range(4):
    for b in range(a+1,4):
        faces=deepcopy(TETRA_FACES)
        for i in [a,b]:faces[i].reverse()
        EXTRA['unify_extra_two_bad_'+str(a)+str(b)]=mesh(TETRA,faces)
EXTRA['unify_extra_repeat_normals']=deepcopy(SDK['unify_normals_ngon_first_outside']);EXTRA['unify_extra_repeat_normals']['normals']=[[1.,0.,0.]]*5
EXTRA['unify_extra_nonmanifold_first']=deepcopy(SDK['unify_normals_nonmanifold']);EXTRA['unify_extra_nonmanifold_first']['faces'][0].reverse()
EXTRA['unify_extra_nonmanifold_last']=deepcopy(SDK['unify_normals_nonmanifold']);EXTRA['unify_extra_nonmanifold_last']['faces'][-1].reverse()
EXTRA['unify_extra_open_fan']=mesh([[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[-1.,0.,0.],[0.,-1.,0.]],[[2,1,0],[0,2,3],[0,3,4]])
EXTRA['unify_extra_boolean_overload']=deepcopy(SDK['unify_normals_custom']);EXTRA['unify_extra_boolean_overload'].update(boolean_overload=True,compute_face_normals=True)
points=[];faces=[]
for i in range(4):
    angle=2.*math.pi*i/4.
    for lateral in [-.25,.25]:
        radius=2.+lateral*math.cos(angle*.5);points.append([radius*math.cos(angle),radius*math.sin(angle),lateral*math.sin(angle*.5)])
for i in range(4):
    a=i*2;b=a+1;na,nb=(1,0)if i==3 else(a+2,b+2);faces.extend([[a,na,b],[na,nb,b]])
EXTRA['unify_extra_mobius']=mesh(points,faces)


HOLDOUTS={}
for a in range(3):
    for b in range(3):
        faces=[[0,1,2],[0,3,2]]
        faces=[f[r:]+f[:r]for f,r in zip(faces,[a,b])]
        HOLDOUTS['unify_holdout_rotation_'+str(a)+str(b)]=mesh(SQUARE,faces)
for count in [2,3,4,5]:
    vertices=[];faces=[]
    for i in range(count):
        vertices.extend([[x+5*i,y,z]for x,y,z in SQUARE])
        faces.extend([[4*i,4*i+3,4*i+2],[4*i,4*i+1,4*i+2]])
    HOLDOUTS['unify_holdout_regions_'+str(count)]=mesh(vertices,faces)
    HOLDOUTS['unify_holdout_regions_reversed_'+str(count)]=mesh(vertices,list(reversed(faces)))
for indices in [[2,3,0,1],[3,2,1,0],[1,0,3,2]]:
    vertices=[None]*4
    for old,new in enumerate(indices):vertices[new]=SQUARE[old]
    HOLDOUTS['unify_holdout_reindex_'+''.join(map(str,indices))]=mesh(vertices,[[indices[v]for v in f]for f in [[0,1,2],[0,3,2]]])

COMMANDS={}
for name in ['pair','consistent','channels','pair_reordered','seam','unused','quads','nonmanifold','duplicate','opposed','ngon','ngon_first_outside','tetra_0','disjoint','tiny','translated']:
    COMMANDS['unify_command_'+name]=scene(name)
COMMANDS['unify_command_mixed']=scene('pair')
COMMANDS['unify_command_mixed']['sources'].append(dict(kind='subd',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2,3]]))
COMMANDS['unify_command_unchanged_peer']=scene('pair')
COMMANDS['unify_command_unchanged_peer']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2],[0,2,3]]))
COMMANDS['unify_command_two_noops']=scene('consistent')
COMMANDS['unify_command_two_noops']['sources'].append(dict(kind='mesh',vertices=[[x+5,y,z]for x,y,z in SQUARE],faces=[[0,1,2],[0,2,3]]))
for name in ['channels','consistent']:
    COMMANDS['unify_command_postselect_'+name]=scene(name,whole=False,preselected=False,command_arguments='_All _Enter')
COMMANDS['unify_command_cancel_picked']=scene('pair',whole=False,preselected=False,command_arguments='_All',cancel=True)
COMMANDS['unify_command_cancel_empty']=scene('pair',whole=False,preselected=False,cancel=True)


def request(discovery=False,extra=False,holdouts=False,commands=False):
    if sum([discovery,extra,holdouts,commands])>1:raise ValueError('one normal unification family required')
    cases=COMMANDS if commands else HOLDOUTS if holdouts else EXTRA if extra else DISCOVERY if discovery else SDK
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='unify_normals_command'if discovery or commands else'unify_normals_sdk',id=k,case=k)for k in cases])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['id']==op['case']and((op['op']=='unify_normals_sdk'and any(op['case']in cases for cases in [SDK,EXTRA,HOLDOUTS]))or(op['op']=='unify_normals_command'and any(op['case']in cases for cases in [DISCOVERY,COMMANDS])))
    except TypeError:valid=False
    if not valid:raise ValueError('fixed normal unification recipe required')
