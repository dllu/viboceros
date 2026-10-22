"""Fixed angle-unwelding cases with explicit stored-normal update policies."""
from copy import deepcopy
from math import pi

BENT=dict(vertices=[[0,0,0],[4,0,0],[0,3,0],[0,0,3],[99,99,99]],faces=[[0,1,2],[1,0,3]])
FLAT=deepcopy(BENT);FLAT['vertices'][3]=[0,-3,0]
REMOTE=deepcopy(BENT)
REMOTE['vertices'][4:]=[[10,0,0],[12,0,0],[10,2,0],[99,99,99]]
REMOTE['faces'].append([4,5,6])
CASES={}
for name,source,angles,channels in [('bent',BENT,[0,89,91,180],'all'),
                                    ('flat',FLAT,[0],'all'),('remote',REMOTE,[0,91],'all'),
                                    ('no_normals',BENT,[0,91],'uv')]:
    for angle in angles:
        for modify in [False,True]:
            case=dict(source=deepcopy(source),angle=angle*pi/180.,channels=channels,modify_normals=modify)
            if channels=='all':
                case['stored_normals']=[[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]][i%3] for i in range(len(source['vertices']))]
                case['transparency']=[(17*i)%256 for i in range(len(source['vertices']))]
            CASES[name+'_'+str(angle)+'_'+('modify' if modify else 'preserve')]=case


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_unweld_channels',id=name,case=name) for name in CASES])


def validate(operation):
    try:
        valid=(isinstance(operation,dict) and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_unweld_channels' and operation['case'] in CASES
               and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed mesh unweld channel recipe required')
