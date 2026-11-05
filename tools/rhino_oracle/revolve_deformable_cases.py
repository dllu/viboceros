"""Fixed public-command experiments for cubic deformable revolutions."""
from copy import deepcopy


def pilot_cases():
    curve = dict(degree=1, knots=[0., 0., 1., 1.], control_points=[
        dict(point=[3., 0., -2.], weight=1.), dict(point=[3., 0., 4.], weight=1.)])
    return [dict(op='revolve_deformable', id=name, curve=deepcopy(curve),
        origin=[0., 0., 0.], axis_end=[0., 0., 1.], start_degrees=start,
        sweep_degrees=sweep, point_count=count, delete_input=False)
        for name, start, sweep, count in [
            ('full_4', 0., 360., 4), ('full_8', 0., 360., 8),
            ('partial_8', 30., 120., 8), ('negative_8', 10., -90., 8)]]


def cases():
    profiles = {
        'line': pilot_cases()[0]['curve'],
        'rational': dict(degree=3, knots=[-2.]*4 + [7.]*4, control_points=[
            dict(point=p, weight=w) for p,w in [([3.,0.,0.],1.),([4.,0.,2.],.5),([2.,0.,5.],2.),([3.,0.,8.],1.)]]),
        'spatial': dict(degree=3, knots=[0.]*4 + [1.]*4, control_points=[
            dict(point=p,weight=1.) for p in [[6.,-2.,-1.],[7.,1.,0.],[3.,-4.,5.],[8.,-3.,4.]]]),
        'axis_tips': dict(degree=3, knots=[0.]*4 + [1.]*4, control_points=[
            dict(point=p,weight=1.) for p in [[0.,0.,0.],[2.,0.,1.],[3.,0.,3.],[0.,0.,5.]]]),
    }
    rows = []
    for profile,curve in profiles.items():
        for count in [4,8,10,16]:
            for sweep in [360.,120.,-270.]:
                rows.append(dict(op='revolve_deformable',id=profile+'_'+str(count)+'_'+str(int(sweep)),
                    curve=deepcopy(curve),origin=[5.,-3.,2.] if profile=='spatial' else [0.,0.,0.],
                    axis_end=[6.,-1.,4.] if profile=='spatial' else [0.,0.,1.],
                    start_degrees=30.,sweep_degrees=sweep,point_count=count,delete_input=count in [4,10],cancel_before_history=False))
    for name,sweep in [('negative_full',-360.),('tiny',.01),('near_full',359.9999)]:
        row=deepcopy(next(row for row in rows if row['id']=='line_8_360'));row.update(id=name,start_degrees=740.,sweep_degrees=sweep)
        rows.append(row)
    return rows


def holdout_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    rows=[]
    def add(name,curve=None,**edits):
        row=deepcopy(template);row.update(id=name,**edits)
        if curve is not None:row['curve']=deepcopy(curve)
        rows.append(row)
    for name,z in [('short_axis',.25),('long_axis',10.),('reversed_axis',-2.)]:
        add(name,axis_end=[0.,0.,z])
    add('midpoint_on_axis',dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[dict(point=p,weight=1.)for p in [[2.,0.,0.],[1.,0.,1.],[-1.,0.,2.],[-2.,0.,3.]]]),axis_end=[0.,0.,4.])
    for sweep in [360.,120.]:
        weighted=deepcopy(template['curve']);weighted['control_points'][1]['weight']=2.;weighted['knots']=[-3.,-3.,7.,7.]
        add('weighted_line_'+str(int(sweep)),weighted,sweep_degrees=sweep)
        multi=dict(degree=2,knots=[0.,0.,0.,.4,.7,1.,1.,1.],control_points=[dict(point=p,weight=w)for p,w in zip([[3.,0.,0.],[4.,0.,1.],[2.,0.,3.],[5.,0.,4.],[3.,0.,6.]],[1.,.75,1.,1.5,1.])])
        add('multispan_'+str(int(sweep)),multi,sweep_degrees=sweep)
        curve=deepcopy(next(row for row in cases() if row['id']=='rational_8_360')['curve'])
        for p in curve['control_points']:p['weight']*=1e10
        add('weight_gauge_'+str(int(sweep)),curve,sweep_degrees=sweep)
        curve=dict(degree=2,knots=[-2.,-1.,0.,.5,1.,2.,3.],control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[4.,0.,2.],[2.,0.,4.],[3.,0.,6.]]])
        add('unclamped_'+str(int(sweep)),curve,sweep_degrees=sweep)
    return rows


def domain_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    rows=[]
    for i,(a,b) in enumerate([(0.,.1),(0.,.5),(0.,2.),(0.,3.),(0.,4.),(0.,5.),(0.,6.),(0.,10.),(0.,100.),(-1.,0.),(-1.,1.),(-3.,7.),(1.,2.),(2.,3.)]):
        row=deepcopy(template);row['id']='profile_domain_'+str(i);row['curve']['knots']=[a,a,b,b];rows.append(row)
    original=next(row for row in holdout_cases() if row['id']=='midpoint_on_axis')
    for name,sweep,reverse,z in [('crossing_partial',120.,False,4.),('crossing_reverse',360.,True,4.),('crossing_short_axis',360.,False,1.),('crossing_negative_axis',360.,False,-4.)]:
        row=deepcopy(original);row.update(id=name,sweep_degrees=sweep,axis_end=[0.,0.,z])
        if reverse:row['curve']['control_points'].reverse()
        rows.append(row)
    return rows


def threshold_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    rows=[]
    for i,b in enumerate([1.19,1.2,1.2000001,1.49,1.5,1.5000001,1.99,17.999,18.,18.001,23.9999,24.,24.0001,29.999,30.,30.0001]):
        row=deepcopy(template);row['id']='threshold_'+str(i);row['curve']['knots']=[0.,0.,b,b];rows.append(row)
    for i,b in enumerate([10.,20.]):
        row=deepcopy(template);row['id']='scaled_threshold_'+str(i);row['curve']['knots']=[0.,0.,b,b]
        for p in row['curve']['control_points']:p['point']=[v*10 for v in p['point']]
        rows.append(row)
    return rows


def scale_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    rows=[]
    pairs=[(6.,d)for d in [1.999999999,2.,2.0001,19.999,20.,20.0001]]+[(h,d)for h in [5.,7.,10.,15.,25.]for d in [1.9,2.,10.,20.]]
    for i,(height,extent) in enumerate(pairs):
        row=deepcopy(template);row['id']='scale_'+str(i);row['curve']['knots']=[0.,0.,extent,extent]
        row['curve']['control_points'][1]['point'][2]=height-2.
        rows.append(row)
    return rows


def policy_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    rows=[]
    pairs=[(3.,d)for d in [18.5,18.8,18.85,18.9,19.,19.5]]+[(r,d)for r in [.1,1.,10.,100.]for d in [1.9,2.,18.5,30.,100.]]
    for i,(radius,extent) in enumerate(pairs):
        row=deepcopy(template);row['id']='policy_'+str(i);row['curve']['knots']=[0.,0.,extent,extent]
        for p in row['curve']['control_points']:p['point'][0]=radius
        rows.append(row)
    return rows


def upper_cases():
    template=deepcopy(next(row for row in cases() if row['id']=='line_8_360'))
    pairs=[(6.,d)for d in [18.01,18.018,18.019,18.1,18.18,18.1817,18.181818,18.1819,18.2]]+[(5.,d)for d in [15.,15.15,15.16]]+[(7.,d)for d in [21.,21.01,21.2,21.2121,21.213]]
    rows=[]
    for i,(height,extent) in enumerate(pairs):
        row=deepcopy(template);row['id']='upper_'+str(i);row['curve']['knots']=[0.,0.,extent,extent]
        row['curve']['control_points'][1]['point'][2]=height-2.;rows.append(row)
    return rows


_OWNED_RECIPES = {row['id']: row for make in [pilot_cases,cases,holdout_cases,domain_cases,threshold_cases,scale_cases,policy_cases,upper_cases] for row in make()}


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v) for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    try:owned=_OWNED_RECIPES.get(op['id']) if isinstance(op,dict) else None
    except (KeyError,TypeError):owned=None
    if owned is None or not same(op,owned):
        raise ValueError('fixed owned deformable revolution required')
