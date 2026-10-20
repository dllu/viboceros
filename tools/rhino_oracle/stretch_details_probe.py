"""Public SDK fitting distances and actual native curve segment structure."""
import math
try:
    from . import twist_command_probe as source
    from . import grip_transform_probe as grips
except (ImportError, ValueError):
    import twist_command_probe as source
    import grip_transform_probe as grips


def validate(op):
    if not isinstance(op, dict) or set(op) != {'op', 'id', 'mode', 'shape', 'axis'} or op['op'] != 'stretch_details':
        raise ValueError('invalid Stretch detail recipe')
    if not isinstance(op['id'], str) or not op['id'].replace('_', '').isalnum():
        raise ValueError('invalid identity')
    if not ((op['mode'] == 'grips' and op['shape'] in ('circle', 'arc') and op['axis'] == 'X') or
            (op['mode'] == 'fit' and op['shape'] in ('Curve', 'Surface', 'Box') and op['axis'] in ('Z', 'Spatial', 'Skew'))):
        raise ValueError('invalid Stretch detail inputs')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('one detail capture required')
    R = host['Rhino']
    if op['mode'] == 'grips':
        doc = R.RhinoDoc.ActiveDoc
        if list(doc.Objects):
            raise ValueError('owned empty document required')
        g = grips.create_source(op['shape'], host)
        try:
            identity = doc.Objects.AddCurve(g)
            obj = doc.Objects.FindId(identity)
            obj.GripsOn = True
            for index in (0, 1):
                obj.GetGrips()[index].Select(True)
            success = R.RhinoApp.RunScript('_Stretch w-1,0,0 w1,0,0 _Copy=No _Rigid=No 2', True)
            result = doc.Objects.FindId(identity).Geometry
            segments = result.DuplicateSegments()
            return dict(success=bool(success), kind=result.GetType().Name,
                        curve=host['_nurbs_curve_definition'](result.ToNurbsCurve()),
                        segments=[dict(kind=s.GetType().Name, domain=[float(s.Domain.T0), float(s.Domain.T1)],
                                       curve=host['_nurbs_curve_definition'](s.ToNurbsCurve())) for s in segments]), 0
        finally:
            g.Dispose()
            for obj in list(doc.Objects):
                obj.GripsOn = False
                doc.Objects.Delete(obj.Id, True)
    axis = {'Z': ([0., 0., 0.], [0., 0., 10.]),
            'Spatial': ([1., 2., 3.], [5., 6., 11.]),
            'Skew': ([1., 2., 3.], [4., -2., 11.])}[op['axis']]
    a, b = [host['_point'](p) for p in axis]
    factor = 2. if op['axis'] == 'Z' else .5
    morph = R.Geometry.Morphs.StretchSpaceMorph(a, b, a.DistanceTo(b)*factor)
    morph.Tolerance = 1e-6
    owned = source.source_geometry(op['shape'], host)
    outputs = []
    try:
        for original in owned:
            result = original.Duplicate()
            try:
                if not morph.Morph(result):
                    raise ValueError('SDK morph failed')
                pairs = [(original, result)] if isinstance(original, R.Geometry.Curve) else list(zip(original.Surfaces, result.Surfaces))
                for before, after in pairs:
                    same, nearest, count = 0., 0., 0
                    is_curve = isinstance(before, R.Geometry.Curve)
                    stations = [(i/512., None) for i in range(513)] if is_curve else [(i/48., j/48.) for i in range(49) for j in range(49)]
                    for u, v in stations:
                        if is_curve:
                            t = before.Domain.ParameterAt(u)
                            exact = morph.MorphPoint(before.PointAt(t))
                            sampled = after.PointAt(after.Domain.ParameterAt(u))
                            ok, closest = after.ClosestPoint(exact)
                            fitted = after.PointAt(closest)
                        else:
                            exact = morph.MorphPoint(before.PointAt(before.Domain(0).ParameterAt(u), before.Domain(1).ParameterAt(v)))
                            sampled = after.PointAt(after.Domain(0).ParameterAt(u), after.Domain(1).ParameterAt(v))
                            ok, cu, cv = after.ClosestPoint(exact)
                            fitted = after.PointAt(cu, cv)
                        if not ok:
                            raise ValueError('closest point failed')
                        same = max(same, exact.DistanceTo(sampled))
                        nearest = max(nearest, exact.DistanceTo(fitted))
                        count += 1
                    outputs.append(dict(samples=count, maximum_same_parameter_error=same, maximum_forward_distance=nearest))
            finally:
                result.Dispose()
        return dict(measurements=outputs), 0
    finally:
        for g in owned:
            g.Dispose()
        morph.Dispose()
