"""Fixed, independent continuity profiles and explicit native split policies."""
from copy import deepcopy


def profiles():
    def curve(degree,points,knots,weights=None):
        return dict(degree=degree,knots=knots,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or [1.]*len(points))])
    knots=[0.,0.,0.,.5,.5,1.,1.,1.]
    first=[[3.,0.,0.],[3.5,0.,1.],[4.25,0.,2.]]
    return dict(
        smooth=curve(2,first+[[5.,0.,3.],[6.,0.,4.]],knots),
        tangent=curve(2,first+[[5.,0.,3.],[6.5,0.,5.]],knots),
        corner=curve(2,first+[[5.25,0.,2.],[5.5,0.,4.]],knots),
        rational=curve(2,[[3.,0.,0.],[4.,0.,1.],[2.,0.,3.],[5.,0.,4.],[3.,0.,6.]],[0.,0.,0.,.4,.7,1.,1.,1.],[1.,.75,1.,1.5,1.]),
        poles=curve(1,[[0.,0.,0.],[3.,0.,1.],[3.,0.,3.],[0.,0.,4.]],[0.,0.,1.,2.,3.,3.]),
        polyline=curve(1,[[3.,0.,0.],[3.,0.,2.],[5.,0.,4.]],[0.,0.,1.,2.,2.]),
    )


def cases():
    rows=[]
    for name,curve in sorted(profiles().items()):
        for deformable in [False,True]:
            for crease in [False,True]:
                for split in [False,True]:
                    rows.append(dict(op='revolve_splitting',id=name+'_'+str(int(deformable))+str(int(crease))+str(int(split)),
                        curve=deepcopy(curve),origin=[0.,0.,0.],axis_end=[0.,0.,1.],start_degrees=30.,sweep_degrees=360.,
                        point_count=8,deformable=deformable,crease_splitting=crease,split_at_tangents=split,delete_input=deformable and split))
    return rows


def holdouts():
    rows=[]
    for row in cases():
        if (row['crease_splitting'],row['split_at_tangents'])not in [(False,True),(True,False)]:continue
        for name,sweep in [('partial',120.),('negative',-270.)]:
            op=deepcopy(row);op.update(id=row['id']+'_'+name,sweep_degrees=sweep);rows.append(op)
    template=cases()[0]
    for index,slope in enumerate([.0008,.0026,.0157,.0192]):
        for deformable in [False,True]:
            for crease in [False,True]:
                for split in [False,True]:
                    op=deepcopy(template);op.update(id='small_'+str(index)+'_'+str(int(deformable))+str(int(crease))+str(int(split)),deformable=deformable,crease_splitting=crease,split_at_tangents=split,delete_input=False)
                    op['curve']=dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[3.,0.,1.],[3.+slope,0.,2.]]]);rows.append(op)
    return rows


def mixed_cases():
    rows=[];template=cases()[0]
    for first in [True,False]:
        points=[[3.,0.,0.],[3.,0.,1.],[3.0026,0.,2.],[4.,0.,2.]]if first else [[3.,0.,0.],[3.,0.,1.],[4.,0.,1.],[5.,0.,1.0026]]
        for deformable in [False,True]:
            for crease in [False,True]:
                for split in [False,True]:
                    for negative in [False,True]:
                        row=deepcopy(template);row.update(id='mixed_'+''.join(str(int(v))for v in [first,deformable,crease,split,negative]),
                            deformable=deformable,crease_splitting=crease,split_at_tangents=split,sweep_degrees=-270. if negative else 360.,delete_input=False)
                        row['curve']=dict(degree=1,knots=[0.,0.,1.,2.,3.,3.],control_points=[dict(point=p,weight=1.)for p in points]);rows.append(row)
    return rows


_OWNED={row['id']:row for row in cases()+holdouts()+mixed_cases()}
def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict)and set(a)==set(b)and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list)and len(a)==len(b)and all(same(x,y)for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool)and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool)and isinstance(a,(int,float))and a==b
        return type(a)==type(b)and a==b
    try:owned=_OWNED.get(op['id'])if isinstance(op,dict)else None
    except (KeyError,TypeError):owned=None
    if owned is None or not same(op,owned):raise ValueError('fixed owned revolution splitting recipe required')
