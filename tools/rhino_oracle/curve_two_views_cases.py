"""Owned Crv2View command grammar and SDK reconstruction controls."""
from copy import deepcopy


def bezier(points):
    return dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[dict(point=p,weight=1.)for p in points])


def discovery():
    curves=[bezier([[0.,0.,0.],[3.,5.,0.],[7.,-2.,0.],[10.,3.,0.]]),
            bezier([[0.,0.,2.],[3.,0.,-3.],[7.,0.,4.],[10.,0.,1.]])]
    rows=[]
    for mode in ['sdk','auto','before','after']:
        rows.append(dict(op='curve_two_views',id='two_views_discovery_'+mode,curves=deepcopy(curves),directions=[[0.,0.,1.],[0.,1.,0.]],mode=mode))
    parallel=deepcopy(rows[0]);parallel['id']='two_views_discovery_parallel';parallel['directions'][1]=[0.,0.,1.];rows.append(parallel)
    return rows


def validate(operation):
    def same(a,b):
        if type(a)is not type(b):return False
        if isinstance(a,dict):return set(a)==set(b)and all(same(a[k],b[k])for k in a)
        if isinstance(a,list):return len(a)==len(b)and all(same(x,y)for x,y in zip(a,b))
        return a==b
    if not any(same(operation,r)for r in discovery()+cases()+audit_cases()+fit_audit_cases()+surface_audit_cases()):raise ValueError('fixed owned Crv2View recipe required')


def cases():
    if __package__:
        from .planar_regions_cases import circle
    else:
        from planar_regions_cases import circle
    from math import cos,sin
    rows=[];base=discovery()[0]
    def add(name,modify=None):
        row=deepcopy(base);row['mode']='before';row['id']='two_views_'+name
        if modify:modify(row)
        rows.append(row);return row
    add('cubic')
    add('reverse_second',lambda r:r['curves'][1]['control_points'].reverse())
    add('domains',lambda r:[r['curves'][i].update(domain=d)for i,d in enumerate([[-20.,-2.],[100.,400.]])])
    for label,weights in [('rational_shared',[1.,.5,2.,1.]),('rational_distinct',[1.,2.,.5,1.])]:
        r=add(label)
        for p,w in zip(r['curves'][0]['control_points'],[1.,.5,2.,1.]):p['weight']=w
        for p,w in zip(r['curves'][1]['control_points'],weights):p['weight']=w
    for label,xs in [('different_clock',[0.,1.,9.,10.]),('partial',[2.,4.,6.,8.]),('disjoint',[20.,23.,27.,30.]),('turning',[0.,-5.,15.,10.])]:
        r=add(label)
        for p,x in zip(r['curves'][1]['control_points'],xs):p['point'][0]=x
    def line(points):return dict(degree=1,knots=[0.,0.,1.,1.],control_points=[dict(point=p,weight=1.)for p in points])
    add('lines',lambda r:r.update(curves=[line([[0.,0.,0.],[10.,5.,0.]]),line([[0.,0.,2.],[10.,0.,4.]])]))
    add('line_and_cubic',lambda r:r['curves'].__setitem__(0,line([[0.,2.,0.],[10.,4.,0.]])))
    for name,radius in [('circles_equal',5.),('circles_unequal',3.)]:
        r=add(name);a=circle(0.,0.,5.);b=circle(0.,0.,radius)
        for p in b['control_points']:p['point']=[p['point'][0],0.,p['point'][1]]
        r['curves']=[a,b]
    add('oblique',lambda r:r.update(directions=[[.2,0.,1.],[0.,1.,.3]]))
    add('opposite',lambda r:r.update(directions=[[0.,0.,-3.],[0.,-4.,0.]]))
    add('parallel',lambda r:r.update(directions=[[0.,0.,1.],[0.,0.,1.]]))
    for name,origin,angle in [('rotated',[2.,-3.,5.],.37),('remote',[1e6,-2e6,3e6],0.)]:
        r=add(name);c,s=cos(angle),sin(angle)
        def rotate(p):x,y,z=p;return [c*x-s*y,s*x+c*y,z]
        for curve in r['curves']:
            for p in curve['control_points']:p['point']=[x+o for x,o in zip(rotate(p['point']),origin)]
        r['directions']=[rotate(d)for d in r['directions']]
    add('auto',lambda r:r.update(mode='auto'))
    add('preselected',lambda r:r.update(mode='preselected',preselect=True))
    def evaluate(curve,t):
        points=[p['point']for p in curve['control_points']]
        while len(points)>1:points=[[a*(1.-t)+b*t for a,b in zip(p,q)]for p,q in zip(points,points[1:])]
        return points[0]
    for label,ranges in [('temporary_first',[[.2,.8],None]),('temporary_both',[[.2,.8],[.2,.8]]),('temporary_reverse',[[.8,.2],[.2,.8]])]:
        r=add(label);r['ranges']=[None if interval is None else [evaluate(curve,t)for t in interval]for curve,interval in zip(r['curves'],ranges)]
    for phase in ['source_first','source_second','direction_base_first','direction_end_first','direction_base_second','direction_end_second']:
        add('cancel_'+phase,lambda r,phase=phase:r.update(cancel=phase))
    return rows


def audit_cases():
    rows=[]
    for label,api,model,angle in [('base',1e-6,1e-6,1e-6),('api_fine',1e-8,1e-6,1e-6),('api_coarse',1e-4,1e-6,1e-6),('model_fine',1e-6,1e-8,1e-6),('angle_fine',1e-6,1e-6,1e-8)]:
        r=deepcopy(discovery()[0]);r.update(id='two_views_audit_'+label,mode='audit',api_tolerance=api,model_tolerance=model,api_angle=angle);rows.append(r)
    return rows


def fit_audit_cases():
    rows=[]
    for scale,label in [(1.,'unit'),(.001,'small'),(1000.,'large')]:
        r=deepcopy(discovery()[0]);r.update(id='two_views_fit_audit_'+label,mode='fit_audit',op='curve_two_views',scale=scale)
        for c in r['curves']:
            for p in c['control_points']:p['point']=[x*scale for x in p['point']]
        rows.append(r)
    return rows


def surface_audit_cases():
    rows=[]
    for extent,label in [(20.,'bounded'),(1e4,'large'),(1e8,'huge')]:
        r=deepcopy(discovery()[0]);r.update(id='two_views_surface_audit_'+label,mode='surface_audit',extent=extent);rows.append(r)
    return rows
