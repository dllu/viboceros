"""Direct, owned section fitting through public MakeCompatible calls."""
import math
import re
import sys

if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if (not isinstance(op, dict)
            or set(op) != {'op', 'id', 'sections', 'parameters', 'refit_tolerance'}
            or op['op'] != 'sweep_refit_sections' or not isinstance(op['id'], str)
            or re.match(r'\A[A-Za-z0-9_.-]{1,100}\Z', op['id']) is None):
        raise ValueError('invalid section fitting recipe')
    sections, parameters, tolerance = op['sections'], op['parameters'], op['refit_tolerance']
    finite = lambda x: type(x) in (int, float) and not math.isnan(x) and not math.isinf(x)
    if (not isinstance(sections, list) or not 1 <= len(sections) <= 256
            or not isinstance(parameters, list) or len(parameters) != len(sections)
            or not all(finite(t) for t in parameters)
            or any(a >= b for a, b in zip(parameters, parameters[1:]))
            or not finite(tolerance) or tolerance < 0.):
        raise ValueError('invalid section fitting count, parameters or tolerance')
    for curve in sections:
        if (not isinstance(curve, dict) or not {'degree', 'knots', 'control_points'} <= set(curve)
                or set(curve) - {'degree', 'knots', 'control_points', 'domain'}
                or type(curve['degree']) is not int or not 1 <= curve['degree'] <= 16
                or not isinstance(curve['control_points'], list)
                or not curve['degree'] + 1 <= len(curve['control_points']) <= 512
                or not isinstance(curve['knots'], list)
                or len(curve['knots']) != len(curve['control_points']) + curve['degree'] + 1):
            raise ValueError('invalid section fitting curve dimensions')
        validate_source(dict(op='geometry_validity', id=op['id'], source=dict(type='nurbs', **curve)))


def run(op, host, iterations, tolerance):
    validate(op)
    geometry = host['Rhino'].Geometry
    sources = []
    try:
        for definition in op['sections']:
            sources.append(host['_nurbs_curve_from_definition'](definition))
        before = [host['_nurbs_curve_definition'](curve) for curve in sources]
        def compute():
            operands, fitted = [], []
            try:
                for source in sources:
                    operands.append(source.DuplicateCurve())
                method = 0 if op['refit_tolerance'] <= 2. * math.sqrt(sys.float_info.epsilon) else 2
                fitted = list(geometry.NurbsCurve.MakeCompatible(
                    operands, geometry.Point3d.Unset, geometry.Point3d.Unset,
                    method, 0, op['refit_tolerance'], math.pi / 180.) or [])
                if len(fitted) != len(sources):
                    raise ValueError('public section fitting returned an incomplete set')
                return [host['_nurbs_curve_definition'](curve) for curve in fitted]
            finally:
                for curve in reversed(fitted):
                    curve.Dispose()
                for curve in reversed(operands):
                    curve.Dispose()
        curves, elapsed = host['_measure'](iterations, compute)
        after = [host['_nurbs_curve_definition'](curve) for curve in sources]
        if before != after:
            raise ValueError('public section fitting changed source definitions')
        return dict(before=before, after=after, curves=curves, parameters=op['parameters']), elapsed
    finally:
        for curve in reversed(sources):
            curve.Dispose()
