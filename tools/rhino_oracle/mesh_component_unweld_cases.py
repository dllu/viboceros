"""Fixed selected-edge and selected-vertex normal-policy experiments."""
from copy import deepcopy
try:
    from .mesh_unweld_channels_cases import BENT,REMOTE
except (ImportError, ValueError):
    from mesh_unweld_channels_cases import BENT,REMOTE

CUBE=dict(vertices=[[0,0,0],[4,0,0],[4,3,0],[0,3,0],[0,0,2],[4,0,2],[4,3,2],[0,3,2],[99,99,99]],
          faces=[[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]])
FAN=dict(vertices=[[0,0,0],[3,0,0],[1,3,0],[-3,1,0],[-1,-3,0],[2,-2,0],[99,99,99]],
         faces=[[0,i+1,(i+1)%5+1]for i in range(5)])
NONMANIFOLD=deepcopy(BENT)
NONMANIFOLD['vertices'][4:]=[[0,-3,0],[99,99,99]]
NONMANIFOLD['faces'].append([0,1,4])
PARTIAL=deepcopy(NONMANIFOLD)
PARTIAL['vertices'][5:]=[[0,0,0],[99,99,99]]
PARTIAL['faces'][-1]=[5,1,4]
UNSHARED=dict(vertices=[BENT['vertices'][i] for f in BENT['faces'] for i in f]+[[99,99,99]],
              faces=[[0,1,2],[3,4,5]])
CASES={}


def add(name,source,kind,indices,channels='all'):
    for modify in [False,True]:
        case=dict(source=deepcopy(source),kind=kind,indices=indices,channels=channels,modify_normals=modify)
        if channels=='all':
            case['stored_normals']=[[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]][i%3]for i in range(len(source['vertices']))]
            case['transparency']=[(17*i)%256 for i in range(len(source['vertices']))]
        CASES[kind+'_'+name+'_'+('modify'if modify else 'preserve')]=case


for kind in ['edge','vertex']:
    for name,source in [('bent',BENT),('remote',REMOTE),('cube',CUBE),('fan',FAN),
                        ('nonmanifold',NONMANIFOLD),('partial',PARTIAL),('unshared',UNSHARED)]:
        add(name,source,kind,[0])
    add('no_normals',BENT,kind,[0],'uv')
    add('naked',BENT,kind,[1]if kind=='edge'else [2])
    add('multiple',CUBE,kind,[0,1,4]if kind=='edge'else [0,2])
    add('duplicate',BENT,kind,[0,0])
    add('empty',BENT,kind,[])


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_unweld_channels',id=name,case=name) for name in CASES])


def validate(operation):
    try:
        valid=(isinstance(operation,dict) and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_component_unweld_channels' and operation['case'] in CASES
               and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed mesh component unweld recipe required')
