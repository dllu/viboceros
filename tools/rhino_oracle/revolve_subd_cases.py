"""Owned control profiles and angular layouts for native SubD revolutions."""
from copy import deepcopy
if __package__:
    from .revolve_splitting_cases import profiles as splitting_profiles
else:
    from revolve_splitting_cases import profiles as splitting_profiles


def pilot():
    source = splitting_profiles()
    source['line'] = dict(degree=1, knots=[0., 0., 5., 5.], control_points=[dict(point=p, weight=1.) for p in [[3., 0., 0.], [3., 0., 5.]]])
    source['cubic'] = dict(degree=3, knots=[0.]*4+[1.]*4, control_points=[dict(point=p, weight=1.) for p in [[3.,0.,0.],[4.,0.,1.],[2.,0.,3.],[3.,0.,5.]]])
    return [dict(op='revolve_subd', id=name+'_'+label, curve=deepcopy(source[name]), origin=[0.,0.,0.], axis_end=[0.,0.,1.], start_degrees=30., sweep_degrees=sweep, segment_count=8, delete_input=False)
            for name,label,sweep in [('line','full',360.),('line','partial',120.),('line','negative',-270.),('cubic','full',360.),('corner','full',360.),('poles','full',360.)]]


def profiles():
    source = splitting_profiles()
    source.update({r['id'].split('_')[0]:r['curve'] for r in pilot()})
    def curve(degree,points,knots,weights=None):
        return dict(degree=degree,knots=knots,control_points=[dict(point=p,weight=w) for p,w in zip(points,weights or [1.]*len(points))])
    source['quadratic']=curve(2,[[2.,0.,0.],[4.,0.,2.],[3.,0.,5.]],[0.]*3+[1.]*3)
    source['quartic']=curve(4,[[2.,0.,0.],[4.,0.,1.],[1.,0.,2.],[5.,0.,4.],[3.,0.,5.]],[0.]*5+[1.]*5)
    source['cubic_multi']=curve(3,[[2.,0.,0.],[4.,0.,1.],[1.,0.,2.],[5.,0.,3.],[2.,0.,4.],[3.,0.,5.]],[0.]*4+[.3,.7]+[1.]*4)
    source['cubic_corner']=curve(3,[[3.,0.,0.],[3.,0.,.3333333333333333],[3.,0.,.6666666666666666],[3.,0.,1.],[3.+.3333333333333333,0.,1.],[3.+.6666666666666666,0.,1.],[4.,0.,1.]],[0.]*4+[1.]*3+[2.]*4)
    source['closed']=curve(1,[[3.,0.,0.],[4.,0.,1.],[3.,0.,2.],[2.,0.,1.],[3.,0.,0.]],[0.,0.,1.,2.,3.,4.,4.])
    return source


def holdouts():
    template=pilot()[0];rows=[]
    for name,curve in sorted(profiles().items()):
        for label,sweep,count in [('full',360.,5),('partial',120.,7),('negative',-270.,4)]:
            row=deepcopy(template);row.update(id=name+'_'+label+'_holdout',curve=deepcopy(curve),sweep_degrees=sweep,segment_count=count,delete_input=label=='negative');rows.append(row)
    for count in [3,4,7,12]:
        row=deepcopy(template);row.update(id='line_full_count_'+str(count),segment_count=count);rows.append(row)
    return rows


def edge_cases():
    template=pilot()[0];rows=[]
    for count in [0,1,2]:
        for sweep in [120.,360.]:
            row=deepcopy(template);row.update(id='line_count_'+str(count)+'_'+str(int(sweep)),segment_count=count,sweep_degrees=sweep);rows.append(row)
    source=profiles()
    source['unclamped']=dict(degree=2,knots=[-2.,-1.,0.,1.,2.,3.,4.,5.],control_points=[dict(point=p,weight=1.) for p in [[2.,0.,0.],[4.,0.,1.],[1.,0.,2.],[5.,0.,3.],[3.,0.,5.]]])
    points=[[3.,0.,0.],[4.,0.,1.],[3.,0.,3.],[2.,0.,2.],[2.,0.,1.]]
    source['periodic']=dict(degree=3,knots=[-2.,-2.,-1.,0.,1.,2.,3.,4.,5.,6.,7.,7.],control_points=[dict(point=p,weight=1.) for p in points+points[:3]])
    for name in ['unclamped','periodic']:
        for sweep in [120.,360.]:
            row=deepcopy(template);row.update(id=name+'_'+str(int(sweep)),curve=source[name],sweep_degrees=sweep);rows.append(row)
    for name in ['line','corner','poles']:
        for label,origin,basis,axis in [
            ('x',[10.,20.,30.],[[0.,1.,0.],[0.,0.,1.],[1.,0.,0.]],[1.,0.,0.]),
            ('spatial',[10.,20.,30.],[[.9428090415820634,-.23570226039551584,-.23570226039551584],[0.,.7071067811865475,-.7071067811865475],[.3333333333333333,.6666666666666666,.6666666666666666]],[1.,2.,2.])]:
            row=deepcopy(template);row.update(id=name+'_'+label,curve=deepcopy(source[name]),origin=origin,axis_end=[a+b for a,b in zip(origin,axis)],sweep_degrees=-270. if name=='corner' else 360.)
            for cv in row['curve']['control_points']:
                p=cv['point'];cv['point']=[origin[i]+sum(p[j]*basis[j][i] for j in range(3)) for i in range(3)]
            rows.append(row)
    for scale,offset,label in [(.0001,[0.,0.,0.],'small'),(100.,[0.,0.,0.],'large'),(1.,[1e6,2e6,-3e6],'translated')]:
        row=deepcopy(template);row.update(id='line_'+label,origin=offset,axis_end=[offset[0],offset[1],offset[2]+1.])
        for cv in row['curve']['control_points']:cv['point']=[a*scale+b for a,b in zip(cv['point'],offset)]
        rows.append(row)
    for name,points in [('contact',[[3.,0.,0.],[0.,0.,1.],[3.,0.,2.]]),('cross',[[3.,0.,0.],[-3.,0.,2.]])]:
        row=deepcopy(template);row.update(id='axis_'+name,curve=dict(degree=1,knots=[0.,0.]+([1.] if len(points)==3 else [])+[2.,2.],control_points=[dict(point=p,weight=1.) for p in points]));rows.append(row)
    return rows


def policy_cases():
    rows=[];source=profiles()
    for name in ['line','cubic_corner','poles','rational','closed']:
        for seed in [False,True]:
            row=deepcopy(pilot()[0]);row.update(id=name+'_policy_'+str(int(seed)),curve=deepcopy(source[name]),crease_splitting=True,seed_surface=seed,delete_input=True);rows.append(row)
    return rows


_OWNED = {r['id']: r for r in pilot()+holdouts()+edge_cases()+policy_cases()}
def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v) for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    try:owned=_OWNED.get(op['id']) if isinstance(op,dict) else None
    except (KeyError,TypeError):owned=None
    if owned is None or not same(op,owned):raise ValueError('fixed owned SubD revolution recipe required')
