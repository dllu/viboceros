"""Fixed allocating edits after face/edge/vertex identity gaps are created."""
from copy import deepcopy
try:
    from .merge_faces_command_cases import STRIP,TRI
except (ImportError,ValueError):
    from merge_faces_command_cases import STRIP,TRI

STRIP3=dict(vertices=STRIP['vertices']+[[6,0,0],[6,2,0]],faces=STRIP['faces']+[[4,6,7,5]])
GRID=dict(vertices=[[2*x,2*y,0]for y in range(3)for x in range(3)],
          faces=[[3*y+x,3*y+x+1,3*y+x+4,3*y+x+3]for y in range(2)for x in range(2)])
DISJOINT=dict(vertices=STRIP['vertices']+[[x+9,y,z]for x,y,z in STRIP['vertices']],
              faces=STRIP['faces']+[[i+6 for i in f]for f in STRIP['faces']])
TWISTED=deepcopy(STRIP3)
TWISTED['vertices']=[[x,y,.125*max(0,x-2)*(1 if y==0 else -1)]for x,y,z in TWISTED['vertices']]
CASES={}
for name,source,picks in [('strip',STRIP,[0,1]),('three-left',STRIP3,[0,1]),('three-right',STRIP3,[1,2]),
                         ('grid-row',GRID,[0,1]),('grid-L',GRID,[0,1,2]),('tri-pair',TRI,[0,1]),
                         ('disjoint',DISJOINT,[0,1,2,3]),('twisted',TWISTED,[0,1])]:
    for local in [False,True]:
        CASES[name+('-local'if local else '-global')]=dict(sources=[deepcopy(source)],faces=picks,local=local)


def validate(op):
    try:
        ok=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_merge_allocation'
            and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:ok=False
    if not ok:raise ValueError('fixed owned SubD allocation case required')
