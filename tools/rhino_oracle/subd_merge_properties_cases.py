"""Fixed face-color and sequential topology-edit witnesses on owned SubDs."""
from copy import deepcopy
try:
    from .merge_faces_command_cases import STRIP, TRI, RING
except (ImportError,ValueError):
    from merge_faces_command_cases import STRIP, TRI, RING

RED=[191,17,43,91]
GREEN=[255,31,193,79]
BLUE=[79,61,97,233]
CASES={}


def add(name,scene,colors,steps):
    CASES[name]=dict(sources=[dict(kind='subd',colors=colors,**deepcopy(scene))],steps=steps)


face=lambda *indices:[dict(kind='face',index=i)for i in indices]
for name,colors in [('first',[RED,None]),('second',[None,GREEN]),('different',[RED,GREEN]),
                    ('same',[RED,RED]),('transparent',[BLUE,RED]),('unset',[None,None])]:
    add('strip-'+name,STRIP,colors,[face(0,1)])
    add('strip-reversed-picks-'+name,STRIP,colors,[face(1,0)])
strip3=dict(vertices=STRIP['vertices']+[[6,0,0],[6,2,0]],faces=STRIP['faces']+[[4,6,7,5]])
add('strip-retain-neighbor',strip3,[RED,GREEN,BLUE],[face(0,1)])
add('strip-retain-first',strip3,[RED,GREEN,BLUE],[face(1,2)])
add('strip-sequential',strip3,[RED,GREEN,BLUE],[face(0,1),face(0,1)])
add('strip-edge',STRIP,[RED,GREEN],[[dict(kind='edge',vertices=[1,2])]])
add('tri-vertex',TRI,[RED,GREEN],[[dict(kind='vertex',index=0)]])
add('single-noop',STRIP,[RED,GREEN],[face(0)])
add('ring-hole-noop',RING,[RED,GREEN,BLUE,None,RED,GREEN,BLUE,None],[face(*range(8))])
add('ring-partial',RING,[RED,GREEN,BLUE,None,RED,GREEN,BLUE,None],[face(0,1,3)])


def validate(op):
    try:
        ok=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_merge_properties'
            and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:ok=False
    if not ok:raise ValueError('fixed owned SubD face-property case required')
