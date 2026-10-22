"""Independent normal reductions, input order and missing-channel controls."""
from copy import deepcopy
try:
    from .mesh_component_weld_cases import BENT,PARTIAL,SHARED,CUBE
except (ImportError, ValueError):
    from mesh_component_weld_cases import BENT,PARTIAL,SHARED,CUBE
CASES={}


def add(kind,name,source,indices,channels='all',normals=None):
    case=dict(source=deepcopy(source),kind=kind,indices=indices,channels=channels)
    if channels=='all':
        case['stored_normals']=deepcopy(normals or [[1.,0.,0.]]*len(source['vertices']))
        case['transparency']=[(17*i)%256 for i in range(len(source['vertices']))]
    CASES['followup_'+kind+'_'+name]=case


for kind in ['edge','vertex']:
    for name,first,other in [('nonunit',[2.,0.,0.],[0.,1.,0.]),
                             ('opposite',[1.,0.,0.],[-1.,0.,0.]),
                             ('zero',[0.,0.,0.],[0.,0.,0.])]:
        normals=[[0.,1.,0.]]*len(BENT['vertices']);normals=deepcopy(normals)
        normals[0]=first;normals[4]=other
        add(kind,name,BENT,[0],normals=normals)
    normals=deepcopy([[0.,0.,1.]]*len(PARTIAL['vertices']))
    normals[0]=[1.,0.,0.];normals[6]=[0.,1.,0.]
    add(kind,'partial_dedup',PARTIAL,[0],normals=normals)
    add(kind,'naked_no_normals',BENT,[1]if kind=='edge'else [2],'uv')
    add(kind,'shared_no_normals',SHARED,[0],'uv')
    add(kind,'empty_no_normals',BENT,[],'uv')
    add(kind,'reverse_selection',CUBE,[2,1,0]if kind=='edge'else [2,0])
    permuted=deepcopy(CUBE);permuted['faces'].reverse()
    add(kind,'reverse_faces',permuted,[0])
add('whole','cube_twin',CUBE,[])


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_weld_followup',id=name,case=name)for name in CASES])


def validate(operation):
    try:
        valid=(isinstance(operation,dict)and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_component_weld_followup'
               and operation['case']in CASES and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed mesh component weld follow-up required')
