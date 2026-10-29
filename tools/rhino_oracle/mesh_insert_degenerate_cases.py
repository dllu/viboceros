"""Authored valid flat-corner quads; coordinates are independent of Native outputs."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source
CASES={}
def add(name,vertices,faces,points):
    CASES[name]=dict(command='InsertPoint',sources=[source('mesh',vertices=deepcopy(vertices),faces=deepcopy(faces))],
        whole=True,preselected=True,live=True,mesh_channels=False,points=points,actions=[dict(click=p)for p in points]+['Enter'])
p=[[0.,0.,0.],[1.,0.,0.],[2.,0.,0.],[0.,2.,0.]]
add('flat-corner',p,[[0,1,2,3]],[[1.,1.,0.]])
add('cyclic-flat-corner',p,[[1,2,3,0]],[[1.,1.,0.]])
add('reversed-flat-corner',p,[[3,2,1,0]],[[1.,1.,0.]])
add('tilted-flat-corner',[[x,y,.1*x+.2*y]for x,y,_ in p],[[0,1,2,3]],[[1.,1.,.3]])
q=[[4.,1.,0.],[5.,0.,0.],[6.,1.,0.],[5.,2.,0.]]
add('untouched-flat-corner',p+q,[[0,1,2,3],[4,5,6,7]],[[5.5,1.5,0.]])
add('both-components',p+q,[[0,1,2,3],[4,5,6,7]],[[1.,1.,0.],[5.5,1.5,0.]])
def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='mesh_insert_degenerate'
           and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case']):
        raise ValueError('fixed owned flat-corner insertion required')
