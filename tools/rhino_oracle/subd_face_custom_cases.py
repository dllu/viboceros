"""Authored material-channel and complete custom UVW witnesses."""
from copy import deepcopy
try:
    from .subd_merge_allocation_cases import STRIP3,GRID,DISJOINT,TWISTED
    from .merge_faces_command_cases import STRIP,TRI,RING
except (ImportError,ValueError):
    from subd_merge_allocation_cases import STRIP3,GRID,DISJOINT,TWISTED
    from merge_faces_command_cases import STRIP,TRI,RING

CASES={}
face=lambda *indices:[dict(kind='face',index=i)for i in indices]


def add(name,source,channels,steps,textures=False,colors=False,continuous=False):
    s=deepcopy(source)
    s['properties']=[dict(material_channel=m,color=([13+17*i,29+13*i,173-11*i,71+9*i]if colors else None))for i,m in enumerate(channels)]
    s['textures']=[([[.25*s['vertices'][v][0]+(0 if continuous else 7*i),
                     -.5*s['vertices'][v][1]+(0 if continuous else 11*i),.125*(j+1)]
                    for j,v in enumerate(f)]if textures else None)for i,f in enumerate(s['faces'])]
    CASES[name]=dict(source=s,steps=steps)


for name,channels in [('first',[17,0]),('second',[0,29]),('different',[17,29]),('same',[17,17]),('max',[65535,65535]),('unset',[0,0])]:
    add('material-'+name,STRIP,channels,[face(0,1)])
    add('material-reverse-'+name,STRIP,channels,[face(1,0)])
add('material-neighbor',STRIP3,[17,29,65535],[face(0,1)])
add('material-first-neighbor',STRIP3,[17,29,65535],[face(1,2)])
add('material-sequential',STRIP3,[17,29,65535],[face(0,1),face(0,1)])
add('material-edge',STRIP,[17,29],[[dict(kind='edge',vertices=[1,2])]])
add('material-vertex',TRI,[17,29],[[dict(kind='vertex',index=0)]])
add('material-noop',STRIP,[17,29],[face(0)])
for name,source,picks in [('strip',STRIP,face(0,1)),('neighbor',STRIP3,face(0,1)),('grid-L',GRID,face(0,1,2)),
                         ('ring-noop',RING,face(*range(8))),('ring-partial',RING,face(0,1,3)),
                         ('disjoint',DISJOINT,face(0,1,2,3)),('twisted',TWISTED,face(0,1))]:
    n=len(source['faces'])
    add('texture-'+name,source,[0]*n,[picks],textures=True)
    add('combined-'+name,source,[17+i for i in range(n)],[picks],textures=True,colors=True)
add('texture-sequential',STRIP3,[0]*3,[face(0,1),face(0,1)],textures=True)
add('texture-continuous',STRIP,[0,0],[face(0,1)],textures=True,continuous=True)
add('combined-edge',STRIP,[17,29],[[dict(kind='edge',vertices=[1,2])]],textures=True,colors=True)
add('combined-vertex',TRI,[17,29],[[dict(kind='vertex',index=0)]],textures=True,colors=True)
for i in [0,1]:
    name='texture-missing-'+str(i)
    add(name,STRIP,[17,29],[face(0,1)],textures=True)
    CASES[name]['source']['textures'][i]=None


def validate(op):
    try:
        ok=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_face_custom'
            and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:ok=False
    if not ok:raise ValueError('fixed owned SubD custom-face case required')
