"""Owned profiles and apex locations for native SubD apex extrusions."""
from copy import deepcopy
if __package__:
    from .revolve_subd_cases import profiles
else:
    from revolve_subd_cases import profiles


def pilot():
    source=profiles()
    return [dict(op='extrude_apex_subd',id=name+('_solid' if solid else ''),curve=deepcopy(source[name]),apex=[0.,3.,2.],solid=solid,delete_input=False)
            for name,solid in [('line',False),('cubic',False),('corner',False),('closed',False),('closed',True),('rational',True)]]


def holdouts():
    source=profiles();rows=[]
    def add(name,curve,apex=[0.,3.,2.],crease=True,delete=False):
        rows.append(dict(op='extrude_apex_subd',id=name,curve=deepcopy(curve),apex=apex,solid=True,delete_input=delete,crease_splitting=crease))
    for name,curve in sorted(source.items()):
        add(name+'_above',curve)
        add(name+'_below',curve,[5.,-4.,7.],delete=True)
    for degree in [2,3,4]:
        source['line_degree_'+str(degree)]=dict(degree=degree,knots=[0.]*(degree+1)+[1.]*(degree+1),control_points=[dict(point=[3.,0.,float(i)],weight=1.)for i in range(degree+1)])
        add('line_degree_'+str(degree),source['line_degree_'+str(degree)])
    source['unclamped']=dict(degree=2,knots=[-2.,-1.,0.,1.,2.,3.,4.,5.],control_points=[dict(point=p,weight=1.)for p in [[2.,0.,0.],[4.,0.,1.],[1.,0.,2.],[5.,0.,3.],[3.,0.,5.]]])
    points=[[3.,0.,0.],[4.,0.,1.],[3.,0.,3.],[2.,0.,2.],[2.,0.,1.]]
    source['periodic']=dict(degree=3,knots=[-2.,-2.,-1.,0.,1.,2.,3.,4.,5.,6.,7.,7.],control_points=[dict(point=p,weight=1.)for p in points+points[:3]])
    points=[[2.,0.,0.],[2.,0.,2.],[0.,0.,2.],[-2.,0.,2.],[-2.,0.,0.],[-2.,0.,-2.],[0.,0.,-2.],[2.,0.,-2.],[2.,0.,0.]]
    source['circle']=dict(degree=2,knots=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.],control_points=[dict(point=p,weight=1. if i%2==0 else .7071067811865476)for i,p in enumerate(points)])
    for name in ['unclamped','periodic','circle']:add(name,source[name])
    for name in ['line','polyline','closed','cubic_corner','corner','circle']:add(name+'_no_crease',source[name],crease=False,delete=True)
    return rows


def diagnostics():
    rows=pilot()[:4]+[row for row in holdouts()if row['id']in ['line_degree_2','line_degree_3','line_degree_4','unclamped','periodic','circle','cubic_corner_above']]
    return [dict(row,op='extrude_apex_subd_sdk',id=row['id']+'_sdk')for row in rows]


def confirmations():
    source=profiles();rows=[]
    def add(name,curve,apex,crease=True,delete=False):
        rows.append(dict(op='extrude_apex_subd',id=name,curve=deepcopy(curve),apex=apex,solid=True,delete_input=delete,crease_splitting=crease,record_brep=True))
    for name in ['line','polyline','closed','poles']:
        for crease in [False,True]:add(name+'_definition_'+str(int(crease)),source[name],[1.,-4.,3.],crease,True)
    for i,slope in enumerate([.0008,.0026,.0157,.0192]):
        curve=dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[dict(point=p,weight=1.)for p in [[3.,0.,0.],[3.,0.,1.],[3.+slope,0.,2.]]])
        add('small_kink_'+str(i),curve,[0.,3.,2.])
    basis=[[.9428090415820634,-.23570226039551584,-.23570226039551584],[0.,.7071067811865475,-.7071067811865475],[.3333333333333333,.6666666666666666,.6666666666666666]]
    def place(p):return [o+sum(p[j]*basis[j][i]for j in range(3))for i,o in enumerate([10.,-30.,20.])]
    for name in ['line','closed','cubic','rational']:
        curve=deepcopy(source[name])
        for cv in curve['control_points']:cv['point']=place(cv['point'])
        add(name+'_spatial_plane',curve,place([0.,3.,2.]),delete=True)
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict) and set(a)==set(b) and all(same(a[k],v) for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list) and len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
        if isinstance(b,bool):return isinstance(a,bool) and a==b
        if isinstance(b,(int,float)):return not isinstance(a,bool) and isinstance(a,(int,float)) and a==b
        return type(a)==type(b) and a==b
    owned=next((r for r in pilot()+holdouts()+diagnostics()+confirmations() if isinstance(op,dict) and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned SubD apex extrusion recipe required')
