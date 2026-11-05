"""Owned profiles and directions for native SubD straight extrusions."""
from copy import deepcopy
if __package__:
    from .revolve_subd_cases import profiles
else:
    from revolve_subd_cases import profiles


def pilot():
    source=profiles();rows=[]
    for name,solid in [('line',False),('cubic',False),('corner',False),('closed',False),('closed',True),('rational',True)]:
        rows.append(dict(op='extrude_curve_subd',id=name+('_solid' if solid else ''),curve=deepcopy(source[name]),direction=[0.,2.,0.],solid=solid,both_sides=False,delete_input=False))
    return rows


def holdouts():
    source=profiles();rows=[]
    def add(name,curve,direction,both=False,delete=False,**extra):
        rows.append(dict(op='extrude_curve_subd',id=name,curve=deepcopy(curve),direction=direction,solid=True,both_sides=both,delete_input=delete,**extra))
    for name,curve in sorted(source.items()):
        add(name+'_positive',curve,[0.,2.,0.])
        add(name+'_oblique_both',curve,[1.,-2.,3.],True,True)
    source['unclamped']=dict(degree=2,knots=[-2.,-1.,0.,1.,2.,3.,4.,5.],control_points=[dict(point=p,weight=1.) for p in [[2.,0.,0.],[4.,0.,1.],[1.,0.,2.],[5.,0.,3.],[3.,0.,5.]]])
    points=[[3.,0.,0.],[4.,0.,1.],[3.,0.,3.],[2.,0.,2.],[2.,0.,1.]]
    source['periodic']=dict(degree=3,knots=[-2.,-2.,-1.,0.,1.,2.,3.,4.,5.,6.,7.,7.],control_points=[dict(point=p,weight=1.) for p in points+points[:3]])
    points=[[2.,0.,0.],[2.,0.,2.],[0.,0.,2.],[-2.,0.,2.],[-2.,0.,0.],[-2.,0.,-2.],[0.,0.,-2.],[2.,0.,-2.],[2.,0.,0.]]
    source['circle']=dict(degree=2,knots=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.],control_points=[dict(point=p,weight=1. if i%2==0 else .7071067811865476)for i,p in enumerate(points)])
    for name in ['unclamped','periodic','circle']:
        add(name+'_positive',source[name],[0.,2.,0.])
        add(name+'_oblique_both',source[name],[1.,-2.,3.],True,True)
    for name in ['line','closed','rational']:
        for label,scale,offset in [('small',.0001,[0.,0.,0.]),('translated',1.,[100.,200.,-300.])]:
            curve=deepcopy(source[name])
            for cv in curve['control_points']:cv['point']=[a*scale+b for a,b in zip(cv['point'],offset)]
            add(name+'_'+label,curve,[0.,2.*scale,0.],True,True)
    spatial=deepcopy(source['cubic'])
    for i,cv in enumerate(spatial['control_points']):cv['point'][1]=float(i%3)
    add('nonplanar',spatial,[1.,2.,3.])
    add('nonplanar_numeric',spatial,[0.,0.,-2.],True,True,distance=-2.)
    for name in ['cubic','closed']:
        for distance in [2.,-3.]:add(name+'_numeric_'+str(int(distance)),source[name],[0.,distance,0.],distance<0.,distance<0.,distance=distance)
    return rows


def diagnostics():
    names=['polyline_oblique_both','smooth_oblique_both','tangent_oblique_both','unclamped_oblique_both','corner_oblique_both','cubic_corner_positive','cubic_corner_oblique_both','closed_oblique_both','circle_positive','periodic_positive']
    return [dict(row,op='extrude_curve_subd_sdk',id=row['id']+'_sdk')for row in holdouts()if row['id']in names]


def confirmations():
    source=profiles();rows=[]
    for name in ['smooth','corner','closed','cubic_corner']:
        curve=deepcopy(source[name]);curve['control_points'].reverse()
        low,high=curve['knots'][0],curve['knots'][-1];curve['knots']=[low+high-k for k in reversed(curve['knots'])]
        for label,direction in [('negative',[0.,-3.,0.]),('oblique',[2.,1.,-1.])]:
            rows.append(dict(op='extrude_curve_subd',id=name+'_reversed_'+label,curve=deepcopy(curve),direction=direction,solid=False,both_sides=False,delete_input=False))
    for name in ['smooth','closed','rational','cubic_corner']:
        curve=deepcopy(source[name]);basis=[[.9428090415820634,-.23570226039551584,-.23570226039551584],[0.,.7071067811865475,-.7071067811865475],[.3333333333333333,.6666666666666666,.6666666666666666]]
        for cv in curve['control_points']:
            p=cv['point'];cv['point']=[o+sum(p[j]*basis[j][i]for j in range(3))for i,o in enumerate([10.,-30.,20.])]
        rows.append(dict(op='extrude_curve_subd',id=name+'_spatial_plane',curve=curve,direction=[1.,-3.,2.],solid=True,both_sides=True,delete_input=True))
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v) for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    owned=next((r for r in pilot()+holdouts()+diagnostics()+confirmations() if isinstance(op,dict) and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned SubD extrusion recipe required')
