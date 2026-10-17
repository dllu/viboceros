"""Bounded public Arc Tangent getters and final arc choice."""
def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    validate_recipe(op,'arc_tangent',{'Tangent','Point','FromFirstPoint','Radius','Enter','Cancel','Center','StartPoint','ThroughPoint','Midpoint','Deformable','Degree','PointCount'},numeric_pattern=r'\A(?:Degree=[0-9]{1,2}|PointCount=(?:[0-9]{1,3}|1000))\Z',allow_points=True,allow_curve_locations=True)
    if not op.get('live_getters'):raise ValueError('Arc Tangent requires owned live input')
    if any(isinstance(a,dict) and 'curve'in a and 'pick'not in a for a in op['actions']):raise ValueError('tangent curve picks require a location')

def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    from conic_getter_input import drive
    diagnostics=[]
    def driver(guides):
        returned,trace=drive(op,host,guides,command='Arc')
        R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
        for obj in doc.Objects:
            if obj.Id in guides or not isinstance(obj.Geometry,G.Curve):continue
            ok,arc=obj.Geometry.TryGetArc()
            if not ok:continue
            reference=G.ArcCurve(G.Circle(arc.Plane,arc.Radius))
            try:
                cuts=[]
                for key in guides:
                    source=doc.Objects.FindId(key).Geometry
                    if not isinstance(source,G.Curve):cuts.append(None);continue
                    events=G.Intersect.Intersection.CurveCurve(reference,source,tolerance['absolute'],tolerance['absolute'])
                    ok,t=source.ClosestPoint(arc.Center)
                    cuts.append(dict(intersections=[host['_xyz'](e.PointA)for e in events],radial_residual=float(source.PointAt(t).DistanceTo(arc.Center)-arc.Radius)if ok else None))
                diagnostics.append(dict(center=host['_xyz'](arc.Center),radius=float(arc.Radius),source_intersections=cuts))
            finally:reference.Dispose()
        return returned,trace
    setup_driver=None
    if 'setup'in op:setup_driver=lambda guides:drive(dict(op,actions=op['setup'],id=op['id']+'-setup'),host,guides,command='Arc')
    result,elapsed=run_primitive(op,host,iterations,tolerance,'Arc',driver,setup_driver)
    result['support_diagnostics']=diagnostics
    return result,elapsed
