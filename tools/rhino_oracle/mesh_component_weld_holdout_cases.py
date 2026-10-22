"""Independent endpoint, adjacent-selection and reversed-face command controls."""
from copy import deepcopy
try:
    from .mesh_component_weld_cases import BENT,CUBE
except (ImportError, ValueError):
    from mesh_component_weld_cases import BENT,CUBE
CASES={}


def add(kind,name,source,indices):
    count=len(source['vertices'])
    CASES['holdout_'+kind+'_'+name]=dict(source=deepcopy(source),kind=kind,indices=indices,channels='all',
        stored_normals=[[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]][i%3]for i in range(count)],
        transparency=[(17*i)%256 for i in range(count)])


add('vertex','bent_second',BENT,[1])
flat=deepcopy(BENT);flat['vertices'][5]=[0,-3,0]
for name,indices in [('flat_first',[0]),('flat_second',[1]),('flat_both',[1,0])]:
    add('vertex',name,flat,indices)
for name,indices in [('cube_second',[1]),('cube_adjacent',[0,1]),
                     ('cube_triple',[0,1,2]),('all_reverse',list(range(7,-1,-1)))]:
    add('vertex',name,CUBE,indices)
reversed_faces=deepcopy(CUBE);reversed_faces['faces'].reverse()
add('vertex','reverse_adjacent',reversed_faces,[0,1])
add('edge','reverse_corner',reversed_faces,[0,1,2])
two_seams=dict(vertices=[[0,0,0],[1,0,0],[0,1,0],[0,0,0],[0,-1,0],
                         [1,0,0],[0,0,0],[0,1,0],[-1,0,0],[99,99,99]],
                faces=[[0,1,2],[3,5,4],[6,8,7]])
add('vertex','inconsistent_winding',two_seams,[0])
reversed_bent=deepcopy(BENT);reversed_bent['faces'].reverse()
add('edge','reverse_bent',reversed_bent,[0])


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_weld_holdout',id=name,case=name)for name in CASES])


def validate(operation):
    try:
        valid=(isinstance(operation,dict)and set(operation)=={'op','id','case'}
               and operation['op']=='mesh_component_weld_holdout'
               and operation['case']in CASES and operation['id']==operation['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed component weld holdout required')
