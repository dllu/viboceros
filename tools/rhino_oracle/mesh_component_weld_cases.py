"""Fixed component-welding commands with independently stored vertex channels."""
from copy import deepcopy

BENT=dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[4,0,0],[0,0,0],[0,0,3],[99,99,99]],
          faces=[[0,1,2],[3,4,5]])
HALF=deepcopy(BENT);HALF['faces'][1]=[3,0,5]
SHARED=dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[0,0,3],[99,99,99]],
            faces=[[0,1,2],[1,0,3]])
NONMANIFOLD=deepcopy(BENT)
NONMANIFOLD['vertices'][6:]=[[0,0,0],[4,0,0],[0,-3,0],[99,99,99]]
NONMANIFOLD['faces'].append([6,7,8])
PARTIAL=deepcopy(NONMANIFOLD);PARTIAL['faces'][1]=[3,0,5]
REMOTE=deepcopy(BENT)
REMOTE['vertices'][6:]=[[10,0,0],[12,0,0],[10,2,0],[99,99,99]]
REMOTE['faces'].append([6,7,8])
CONTACT=dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[0,0,0],[-3,0,0],[0,-2,1],[99,99,99]],
             faces=[[0,1,2],[3,4,5]])
POINTS=[[0,0,0],[4,0,0],[4,3,0],[0,3,0],[0,0,2],[4,0,2],[4,3,2],[0,3,2]]
CUBE=dict(vertices=deepcopy(POINTS)*3+[[99,99,99]],faces=[])
uses=[0]*8
for face in [[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]]:
    CUBE['faces'].append([i+8*uses[i]for i in face])
    for i in face:uses[i]+=1
CASES={}


def add(name,source,kind,indices,channels='all',normal_mode='axes'):
    source=deepcopy(source)
    case=dict(source=source,kind=kind,indices=indices,channels=channels)
    if channels=='all':
        case['stored_normals']=[([1.,0.,0.]if normal_mode=='x'else
                                 [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]][i%3])
                                for i in range(len(source['vertices']))]
        case['transparency']=[(17*i)%256 for i in range(len(source['vertices']))]
    CASES[kind+'_'+name]=case


for kind in ['edge','vertex']:
    for name,source in [('bent',BENT),('half_shared',HALF),('shared',SHARED),
                        ('nonmanifold',NONMANIFOLD),('partial',PARTIAL),
                        ('remote',REMOTE),('contact',CONTACT),('cube',CUBE)]:
        add(name,source,kind,[0])
    add('no_normals',BENT,kind,[0],'uv')
    add('custom_x',BENT,kind,[0],normal_mode='x')
    add('cube_corner',CUBE,kind,[0,1,2]if kind=='edge'else [0,2])
    add('cube_all',CUBE,kind,list(range(12 if kind=='edge'else 8)))
    add('naked',BENT,kind,[1]if kind=='edge'else [2])
    add('duplicate',BENT,kind,[0,0])
    add('empty',BENT,kind,[])


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_weld_commands',id=name,case=name)for name in CASES])


def validate(operation):
    try:
        valid=(isinstance(operation,dict)and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_component_weld_commands'
               and operation['case']in CASES and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed mesh component weld recipe required')
