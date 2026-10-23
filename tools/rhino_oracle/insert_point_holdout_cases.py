"""Spatial, n-gon, raw-seam and newly-created-edge insertion holdouts."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source
QUAD=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]]
CASES={}
def case(name,shape,points,channels=False):
    CASES['insert_point_holdout_'+name]=dict(command='InsertPoint',sources=[shape],whole=True,preselected=True,live=True,mesh_channels=channels,points=points,actions=[dict(click=p)for p in points]+['Enter'])
for kind in ['mesh','subd']:
    q=deepcopy(QUAD);q[2][2]=1.
    case(kind+'_warped',source(kind,vertices=q,faces=[[0,1,2,3]]),[[0.,1.5,0.],[4.,1.5,0.5]])
    case(kind+'_triangle',source(kind,vertices=QUAD[:2]+[QUAD[3]],faces=[[0,1,2]]),[[0.,1.5,0.],[2.,1.5,0.]])
    case(kind+'_created_edge',source(kind,vertices=QUAD,faces=[[0,1,2,3]]),[[0.,1.5,0.],[4.,1.5,0.],[2.,1.5,0.]])
ngon=source('mesh',vertices=QUAD,faces=[[0,1,2],[0,2,3]])
ngon['ngons']=[dict(vertices=[0,1,2,3],faces=[0,1])]
case('mesh_ngon_one',deepcopy(ngon),[[0.,1.5,0.]])
case('mesh_ngon_two',deepcopy(ngon),[[0.,1.5,0.],[4.,1.5,0.]])
case('mesh_ngon_internal',deepcopy(ngon),[[2.,1.5,0.]])
island=deepcopy(ngon);island['vertices'] += [[x+8.,y,z]for x,y,z in QUAD];island['faces'] += [[4,5,6,7]]
case('mesh_ngon_island',island,[[8.,1.5,0.]])
seam=source('mesh',vertices=[[0.,0.,0.],[2.,0.,0.],[2.,3.,0.],[0.,3.,0.],[2.,0.,0.],[4.,0.,0.],[4.,3.,0.],[2.,3.,0.]],faces=[[0,1,2,3],[4,5,6,7]])
seam.update(texture_coordinates=[[0.,0.],[1.,0.],[1.,1.],[0.,1.],[10.,0.],[11.,0.],[11.,1.],[10.,1.]],normals=[[0.,0.,1.]]*4+[[0.,1.,0.]]*4)
case('mesh_seam',seam,[[0.,1.5,0.],[4.,1.5,0.]],True)
case('mesh_reverse',source('mesh',vertices=QUAD,faces=[[0,1,2,3]]),[[2.,3.,0.],[0.,1.5,0.]])
