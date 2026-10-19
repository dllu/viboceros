"""Public closest-parameter queries beside actual surface-control removal."""
try:
    from . import remove_control_point_probe as base
except (ImportError, ValueError):
    import remove_control_point_probe as base


def plain(op):
    return dict(op, op='remove_control_point_command')


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'remove_control_point_parameters':
        raise ValueError('invalid control parameter recipe')
    base.validate(plain(op))
    if op['family'] != 'surface' or len(op['actions']) != 1 or 'point' not in op['actions'][0]:
        raise ValueError('parameter queries require one world point on a surface source')


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    rhino = base.base.Forward(R); app = base.base.Forward(R.RhinoApp); rhino.RhinoApp = app
    state = {}

    def execute(macro, echo):
        if not macro.startswith('_RemoveControlPoint '):
            return R.RhinoApp.RunScript(macro, echo)
        surface = list(doc.Objects)[0].Geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
        curves = []
        try:
            point = G.Point3d(*op['actions'][0]['point'])
            found, u, v = surface.ClosestPoint(point)
            state['surface_closest'] = dict(found=bool(found), parameters=[u, v])
            projected = surface.PointAt(u,v)
            state['projected_point'] = host['_xyz'](projected)
            face = list(doc.Objects)[0].Geometry.Faces[0]
            face_ok, face_u, face_v = face.ClosestPoint(point)
            state['face_closest'] = dict(found=bool(face_ok), parameters=[face_u,face_v])
            state['curves'] = []
            for axis, other in ((0, v), (1, u)):
                iso = surface.IsoCurve(axis, other); curves.append(iso)
                n = iso.ToNurbsCurve(); curves.append(n)
                ok, t = n.ClosestPoint(point)
                projected_ok, projected_t = n.ClosestPoint(projected)
                state['curves'].append(dict(kind='iso', axis=axis, parameters=[t],
                    projected_parameter=projected_t, projected_found=bool(projected_ok),
                    greville=[float(n.GrevilleParameter(i)) for i in range(n.Points.Count)],
                    domain=[n.Domain.T0, n.Domain.T1], found=bool(ok)))
                fixed_count = surface.Points.CountV if axis == 0 else surface.Points.CountU
                length = surface.Points.CountU if axis == 0 else surface.Points.CountV
                knots = list(surface.KnotsU if axis == 0 else surface.KnotsV)
                knots = [knots[0]] + knots + [knots[-1]]
                degree = surface.Degree(axis)
                for row in range(fixed_count):
                    cvs = [surface.Points.GetControlPoint(i, row) if axis == 0 else surface.Points.GetControlPoint(row, i) for i in range(length)]
                    definition = dict(degree=degree, knots=knots,
                        control_points=[dict(point=host['_xyz'](cv.Location), weight=cv.Weight) for cv in cvs])
                    n = host['_nurbs_curve_from_definition'](definition); curves.append(n)
                    ok, t = n.ClosestPoint(point)
                    q = n.PointAt(t)
                    state['curves'].append(dict(kind='net', axis=axis, row=row, parameters=[t],
                        greville=[float(n.GrevilleParameter(i)) for i in range(n.Points.Count)],
                        domain=[n.Domain.T0,n.Domain.T1], distance=q.DistanceTo(point), found=bool(ok)))
            unit = surface.ToNurbsSurface()
            curves.append(unit)
            if not unit.SetDomain(0,G.Interval(0.,1.)) or not unit.SetDomain(1,G.Interval(0.,1.)):
                raise ValueError('unit parameter query preparation failed')
            ok, unit_u, unit_v = unit.ClosestPoint(point)
            state['unit_surface'] = dict(found=bool(ok), parameters=[unit_u,unit_v])
            unit_projected=unit.PointAt(unit_u,unit_v)
            state['unit_curves'] = []
            for axis, other in ((0,unit_v),(1,unit_u)):
                curve=unit.IsoCurve(axis,other);curves.append(curve)
                n=curve.ToNurbsCurve();curves.append(n)
                ok,t=n.ClosestPoint(point)
                projected_ok,projected_t=n.ClosestPoint(unit_projected)
                state['unit_curves'].append(dict(axis=axis,parameter=t,found=bool(ok),
                    projected_parameter=projected_t,projected_found=bool(projected_ok),
                    greville=[float(n.GrevilleParameter(i)) for i in range(n.Points.Count)]))
        finally:
            for curve in reversed(curves):
                if curve is not None:
                    curve.Dispose()
            surface.Dispose()
        return R.RhinoApp.RunScript(macro, echo)

    app.RunScript = execute
    local = dict(host); local['Rhino'] = rhino
    value, elapsed = base.run(plain(op), local, iterations, tolerance)
    value.update(parameter_queries=state, recipe=op)
    return value, elapsed
