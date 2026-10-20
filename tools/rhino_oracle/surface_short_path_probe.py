"""Owned public Surface.ShortPath observations on fixed source surfaces."""
import re
from copy import deepcopy
CASES={
    'plane':([.1,.2],[.9,.8]),'skew_plane':([.1,.2],[.9,.8]),
    'cylinder':([.15,.2],[.4,.8]),'cylinder_seam':([.95,.2],[.05,.8]),'cylinder_half':([0.,.2],[.5,.8]),
    'sphere':([.1,.35],[.4,.75]),'sphere_pole':([.1,.5],[.3,1.]),'sphere_antipodal':([0.,.5],[.5,.5]),
    'warped':([.15,.2],[.85,.8]),'general':([.2,.25],[.8,.75]),'rational':([.2,.25],[.8,.75]),
    'boundary':([0.,.2],[.8,.7]),'chart':([.2,.25],[.8,.75]),'coincident':([.2,.3],[.2,.3]),
}
def request():
    return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='surface_short_path',id=case,case=case,start=a,end=b)for case,(a,b)in CASES.items()]))
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','start','end'}or op['op']!='surface_short_path':raise ValueError('invalid short path observation')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or not isinstance(op['case'],str)or op['case']not in CASES:raise ValueError('invalid owned short path case')
    if (op['start'],op['end'])!=CASES[op['case']]:raise ValueError('only fixed owned short path endpoints are allowed')
def validate_request(q):
    if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=16:raise ValueError('bounded one-iteration short path observations required')
    for op in q['operations']:validate(op)
    if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('short path ids must be unique')
def source(case,host):
    G=host['Rhino'].Geometry
    if case.startswith('cylinder'):return G.Cylinder(G.Circle(G.Plane.WorldXY,3.),6.).ToNurbsSurface()
    if case.startswith('sphere'):return G.Sphere(G.Point3d.Origin,4.).ToNurbsSurface()
    if case in('plane','skew_plane','warped','coincident'):
        points=[[0,0,0],[2,0,0],[3,4,0],[1,4,0]]if case=='skew_plane'else[[-5,-5,0],[5,-5,0],[5,5,3],[-5,5,-1]]if case=='warped'else[[-5,-5,0],[5,-5,0],[5,5,0],[-5,5,0]]
        return G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in points])
    definition=dict(type='surface',degree_u=2,degree_v=2,control_point_count_u=3,control_point_count_v=3,knots_u=[0.,0.,0.,1.,1.,1.],knots_v=[0.,0.,0.,1.,1.,1.],control_points=[dict(point=[float(u),float(v),.2*u*v*v],weight=1.1 if case=='rational'and u==v==1 else 1.)for v in range(3)for u in range(3)])
    surface=host['_object_source'](definition,1e-9)
    if case=='chart':
        if not surface.SetDomain(0,G.Interval(-2.,3.))or not surface.SetDomain(1,G.Interval(1e6,1e6+2.)):surface.Dispose();raise ValueError('owned chart change failed')
    return surface
def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one short path observation required')
    G=host['Rhino'].Geometry;surface=source(op['case'],host);curve=None;nurbs=None
    try:
        if surface is None or not surface.IsValid:raise ValueError('invalid owned short path surface')
        before=int(surface.DataCRC(0));a,b=[G.Point2d(surface.Domain(0).ParameterAt(p[0]),surface.Domain(1).ParameterAt(p[1]))for p in[op['start'],op['end']]]
        error=None
        try:curve=surface.ShortPath(a,b,float(tolerance['absolute']))
        except host['System'].ArgumentException as exception:error=str(exception)
        if curve is not None:
            if not curve.IsValid:raise ValueError('invalid native short path curve')
            nurbs=curve.ToNurbsCurve()
        if int(surface.DataCRC(0))!=before:raise ValueError('ShortPath mutated its source')
        return dict(surface=host['_nurbs_surface_definition'](surface),start=[float(a.X),float(a.Y)],end=[float(b.X),float(b.Y)],points=[host['_xyz'](surface.PointAt(p.X,p.Y))for p in[a,b]],source_unchanged=True,curve=host['_nurbs_curve_definition'](nurbs)if nurbs is not None else None,length=float(curve.GetLength())if curve is not None else None,api_error=error),0
    finally:
        if nurbs is not None:nurbs.Dispose()
        if curve is not None:curve.Dispose()
        if surface is not None:surface.Dispose()
