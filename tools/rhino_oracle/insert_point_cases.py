"""Fixed mesh/SubD edge insertion paths and local Undo recipes."""
from copy import deepcopy
if __package__:
    from .insert_point_spatial_layout_cases import CASES as SPATIAL, SEQUENCES
    from .insert_point_holdout_cases import CASES as HOLDOUTS
    from .insert_point_ngon_cases import CASES as NGONS
    from .insert_point_mesh_shape_cases import CASES as SHAPES
    from .insert_point_pick_depth_cases import CASES as DEPTH
    from .insert_point_pick_depth_cases import WITNESSES
else:
    from insert_point_spatial_layout_cases import CASES as SPATIAL, SEQUENCES
    from insert_point_holdout_cases import CASES as HOLDOUTS
    from insert_point_ngon_cases import CASES as NGONS
    from insert_point_mesh_shape_cases import CASES as SHAPES
    from insert_point_pick_depth_cases import CASES as DEPTH
    from insert_point_pick_depth_cases import WITNESSES
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source
QUAD=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]]
DISCOVERY={}
for kind in ['mesh','subd']:
    for count in [1,2]:
        points=[[0.,1.5,0.],[4.,1.5,0.]][:count]
        shape=source(kind,vertices=QUAD,faces=[[0,1,2,3]],corners=[0,1,2,3]if kind=='subd'else[])
        DISCOVERY['insert_point_'+kind+'_'+str(count)]=dict(command='InsertPoint',sources=[shape],whole=True,preselected=True,
            command_arguments=' '.join('w'+','.join(str(x)for x in p)for p in points),points=points)
CASES=dict(DISCOVERY)
LIVE={}
for key,recipe in DISCOVERY.items():
    row=deepcopy(recipe);row['live']=True;row['actions']=[dict(click=p)for p in row['points']]+['Enter'];row.pop('command_arguments')
    LIVE[key.replace('insert_point_','insert_point_live_')]=row
for kind in ['mesh','subd']:
    base=deepcopy(DISCOVERY['insert_point_'+kind+'_2']);base.pop('command_arguments');base['live']=True
    for name,tail in [('undo',['Undo','Enter']),('cancel',['Cancel'])]:
        row=deepcopy(base);row['actions']=[dict(click=p)for p in row['points']]+tail;LIVE['insert_point_live_'+kind+'_'+name]=row
FULL={}
for kind in ['mesh','subd']:
    shapes={
        'adjacent':(source(kind,vertices=QUAD,faces=[[0,1,2,3]],corners=[0,1,2,3]if kind=='subd'else[]),[[0.,1.5,0.],[2.,3.,0.]]),
        'three':(source(kind,vertices=QUAD,faces=[[0,1,2,3]],corners=[0,1,2,3]if kind=='subd'else[]),[[0.,1.5,0.],[4.,1.5,0.],[2.,3.,0.]]),
        'same_edge':(source(kind,vertices=QUAD,faces=[[0,1,2,3]],corners=[0,1,2,3]if kind=='subd'else[]),[[0.,.75,0.],[0.,2.25,0.]]),
        'diagonal':(source(kind,vertices=QUAD,faces=[[0,1,2,3]],corners=[0,1,2,3]if kind=='subd'else[]),[[0.,0.,0.],[4.,3.,0.]]),
        'smooth':(source(kind,vertices=QUAD,faces=[[0,1,2,3]]),[[0.,1.5,0.],[4.,1.5,0.]]),
        'strip_mid':(source(kind,vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],corners=[0,2,3,5]if kind=='subd'else[]),[[2.,1.5,0.]]),
        'strip_cross':(source(kind,vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],corners=[0,2,3,5]if kind=='subd'else[]),[[0.,1.5,0.],[2.,1.5,0.],[4.,1.5,0.]]),
        'strip_skip':(source(kind,vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],corners=[0,2,3,5]if kind=='subd'else[]),[[0.,1.5,0.],[4.,1.5,0.]]),
    }
    for name,(shape,points)in shapes.items():
        FULL['insert_point_full_'+kind+'_'+name]=dict(command='InsertPoint',sources=[shape],whole=True,preselected=True,live=True,points=points,actions=[dict(click=p)for p in points]+['Enter'])
    for name,tail in [('undo_all',['Undo','Undo','Enter']),('cancel_empty',['Cancel'])]:
        row=deepcopy(LIVE['insert_point_live_'+kind+'_2']);row['actions']=[dict(click=p)for p in row['points']]+tail if name=='undo_all'else['Cancel'];FULL['insert_point_full_'+kind+'_'+name]=row
row=deepcopy(LIVE['insert_point_live_mesh_2']);shape=row['sources'][0]
shape.update(texture_coordinates=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[1.,0.,0.]]*4,colors=[[255,0,0,255],[0,255,0,128],[0,0,255,0],[90,80,70,255]],ngons=[dict(vertices=[0,1,2,3],faces=[0])])
row['mesh_channels']=True;FULL['insert_point_full_mesh_channels']=row
REGIONS={}
for side in range(4):
    for fraction in [.25,.75]:
        a=QUAD[side];b=QUAD[(side+1)%4];p=[x+(y-x)*fraction for x,y in zip(a,b)]
        row=deepcopy(LIVE['insert_point_live_mesh_1']);row['points']=[p];row['actions']=[dict(click=p),'Enter'];REGIONS['insert_point_regions_side_'+str(side)+'_'+str(int(100*fraction))]=row
for name,points in [('low',[[0.,.3,0.],[2.,3.,0.]]),('high',[[0.,2.7,0.],[2.,3.,0.]]),('reverse',[[2.,3.,0.],[0.,1.5,0.]]),('same_reverse',[[0.,2.25,0.],[0.,.75,0.]])]:
    row=deepcopy(LIVE['insert_point_live_mesh_2']);row['points']=points;row['actions']=[dict(click=p)for p in points]+['Enter'];REGIONS['insert_point_regions_'+name]=row
ALL_CASES = {}
for family in (CASES, LIVE, FULL, REGIONS, HOLDOUTS, NGONS, SHAPES, DEPTH, WITNESSES, SPATIAL, SEQUENCES):
    ALL_CASES.update(family)
def request(live=False,full=False,regions=False,holdouts=False,ngons=False,shapes=False,depth=False,witnesses=False,spatial=False,sequences=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='insert_point_command',id=k,case=k)for k in (SEQUENCES if sequences else SPATIAL if spatial else WITNESSES if witnesses else DEPTH if depth else SHAPES if shapes else NGONS if ngons else HOLDOUTS if holdouts else REGIONS if regions else FULL if full else LIVE if live else CASES)])
def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='insert_point_command'and op['case']in ALL_CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed InsertPoint recipe required')
