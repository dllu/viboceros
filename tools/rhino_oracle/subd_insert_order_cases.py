"""Independent reverse/prefix witnesses for InsertPoint allocation and rejection."""
from copy import deepcopy

CASES={}
def add(name,seed,points,tail=None,merge=None):
    CASES[name]=dict(seed=seed,asset='base',points=deepcopy(points),
                    actions=[dict(click=p)for p in points]+(tail or ['Enter']),merge=merge)

L=[0.,1.,0.];M=[2.,1.,0.];R=[4.,1.,0.];F=[6.,1.,0.];D=[13.,1.,0.]
add('reverse-cross','combined-strip',[R,L])
add('right-prefix','combined-strip',[R])
add('reverse-three','combined-strip',[R,M,L])
add('cross-undo-prefix','combined-strip',[R,L],['Undo','Enter'])
add('merged-left-prefix','combined-neighbor',[L],merge=[0,1])
add('merged-reverse-cross','combined-neighbor',[F,L],merge=[0,1])
add('merged-reverse-three','combined-neighbor',[F,R,L],merge=[0,1])
add('merged-cross-undo','combined-neighbor',[L,F],['Undo','Enter'],merge=[0,1])
add('disjoint-left-prefix','combined-disjoint',[L],merge=[0,1])
add('disjoint-reverse','combined-disjoint',[D,L],merge=[0,1])
add('disjoint-return','combined-disjoint',[L,D,R],merge=[0,1])
add('disjoint-reject-undo','combined-disjoint',[L,D],['Undo','Enter'],merge=[0,1])
add('grid-two','combined-grid-L',[L,R])
add('grid-reverse-three','combined-grid-L',[[3.,4.,0.],R,L])
add('merged-grid-reverse','combined-grid-L',[[4.,3.,0.],L],merge=[0,1,2])
add('same-edge-reverse','combined-strip',[[0.,1.5,0.],[0.,.5,0.]])


def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_insert_order'
           and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case']):
        raise ValueError('fixed owned SubD InsertPoint allocation holdout required')
