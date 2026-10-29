"""Fixed material/UVW and allocator witnesses for actual InsertPoint getters."""
from copy import deepcopy

CASES={}


def add(name,seed,points,tail=None,merge=None,asset='base'):
    actions=[dict(click=p)for p in points]+(tail or ['Enter'])
    CASES[name]=dict(seed=seed,asset=asset,points=deepcopy(points),actions=actions,merge=merge)


left=[0.,1.,0.];middle=[2.,1.,0.];right=[4.,1.,0.]
for label,points in [('boundary',[left]),('interior',[middle]),('face',[left,middle]),
                     ('cross',[left,right]),('three',[left,middle,right]),
                     ('adjacent',[left,[1.,2.,0.]]),('same-edge',[[0.,.5,0.],[0.,1.5,0.]])]:
    add('combined-'+label,'combined-strip',points)
add('combined-local-undo','combined-strip',[left,right],['Undo','Enter'])
add('combined-undo-all','combined-strip',[left,right],['Undo','Undo','Enter'])
add('combined-cancel','combined-strip',[left,right],['Cancel'])
add('material-cross','material-neighbor',[left,[6.,1.,0.]])
add('material-same','material-same',[left,right])
add('texture-cross','texture-strip',[left,right])
add('texture-missing','texture-missing-0',[left,right])
add('coherent-three','coherent-strip',[left,middle,right],asset='holdout')
add('constant-cross','constant-uvw',[left,right],asset='holdout')
add('pentagon-cross','pentagon-and-quad',[left,right],asset='holdout')
add('grid-three','combined-grid-L',[[0.,1.,0.],[4.,1.,0.],[3.,4.,0.]])
add('merged-neighbor','combined-neighbor',[left,[6.,1.,0.]],merge=[0,1])
add('merged-neighbor-three','combined-neighbor',[left,[4.,1.,0.],[6.,1.,0.]],merge=[0,1])
add('merged-grid','combined-grid-L',[left,[4.,3.,0.]],merge=[0,1,2])
add('merged-disjoint','combined-disjoint',[left,[13.,1.,0.]],merge=[0,1])


def validate(op):
    try:
        valid=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_insert_custom'
               and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed owned SubD InsertPoint custom-data case required')
