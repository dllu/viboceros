"""Authored multi-pick edge paths; no output-derived coordinates or tessellations."""
from copy import deepcopy
from math import cos,sin
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source

CASES={}
def add(name,vertices,edge=3,parameters=(.2,.8),face=None,ngon=False):
    points=[[a+(b-a)*t for a,b in zip(vertices[edge],vertices[(edge+1)%len(vertices)])]for t in parameters]
    f=face or list(range(len(vertices)))
    shape=source('mesh',vertices=deepcopy(vertices),faces=[f])
    if ngon:shape['ngons']=[dict(vertices=f,faces=[0])]
    CASES[name]=dict(command='InsertPoint',sources=[shape],whole=True,preselected=True,live=True,
                    mesh_channels=False,points=points,actions=[dict(click=p)for p in points]+['Enter'])

FLAT=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]]
WARP=[[0.,0.,0.],[4.,0.,0.],[4.,3.,1.2],[0.,3.,.4]]
for label,quad in [('flat',FLAT),('warped',WARP)]:
    for edge in range(4):add(label+'-edge-'+str(edge),quad,edge)
    add(label+'-three',quad,parameters=(.15,.45,.85))
    add(label+'-reverse',quad,parameters=(.8,.2))
    add(label+'-same-low',quad,parameters=(.65,.85))
    add(label+'-same-high',quad,parameters=(.15,.35))
add('warped-reversed-face',WARP,face=[0,3,2,1])
add('warped-cyclic-face',WARP,face=[1,2,3,0])
add('warped-ngon',WARP,ngon=True)
add('skewed-flat',[[0.,0.,0.],[6.,1.,0.],[4.,4.,0.],[-1.,3.,0.]])
add('trapezoid',[[0.,0.,0.],[5.,0.,0.],[4.,3.,0.],[1.,3.,0.]])
add('tall-warp',[[0.,0.,0.],[2.,0.,0.],[2.,6.,1.2],[0.,6.,.4]])
add('wide-warp',[[0.,0.,0.],[8.,0.,0.],[8.,2.,.5],[0.,2.,-.2]])
def rotate(p):
    x,y,z=p
    return [3.+x*cos(.4)-y*sin(.4),-2.+x*sin(.4)+y*cos(.4),z]
add('rotated-warp',[rotate(p)for p in WARP])
add('triangle',[[0.,0.,0.],[4.,0.,0.],[0.,3.,.4]],edge=2)
add('triangle-ngon',[[0.,0.,0.],[4.,0.,0.],[0.,3.,.4]],edge=2,ngon=True)

PATH_CASES=tuple(CASES)
DIAMOND=[[0.,1.5,0.],[2.,0.,0.],[4.,1.5,0.],[2.,3.,0.]]
DISCONNECTED={}
for name,points,tail in [
    ('islands-reject',[[1.,2.25,0.],[9.,2.25,0.]],['Enter']),
    ('islands-reverse',[[9.,2.25,0.],[1.,2.25,0.]],['Enter']),
    ('islands-return',[[1.,2.25,0.],[9.,2.25,0.],[3.,.75,0.]],['Enter']),
    ('islands-undo',[[1.,2.25,0.],[9.,2.25,0.]],['Undo','Enter']),
]:
    vertices=deepcopy(DIAMOND)+[[x+6.,y,z]for x,y,z in DIAMOND]
    DISCONNECTED[name]=dict(command='InsertPoint',sources=[source('mesh',vertices=vertices,faces=[[0,1,2,3],[4,5,6,7]])],
        whole=True,preselected=True,live=True,mesh_channels=False,points=points,
        actions=[dict(click=p)for p in points]+tail)
CASES.update(DISCONNECTED)

def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='mesh_insert_paths'
           and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case']):
        raise ValueError('fixed owned mesh InsertPoint path required')
