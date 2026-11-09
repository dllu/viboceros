"""Read-only public Surface.Fit observations on owned NURBS tensors."""
import math


def validate(op):
    required = {'op', 'id', 'surface', 'degree_u', 'degree_v', 'fit_tolerance'}
    if (not isinstance(op, dict) or set(op) != required
            or op['op'] != 'surface_fit' or not isinstance(op['id'], str)
            or not isinstance(op['surface'], dict)):
        raise ValueError('invalid surface fit operation')
    if any(type(op[k]) is not int or not 1 <= op[k] <= 11 for k in ['degree_u', 'degree_v']):
        raise ValueError('invalid surface fit degree')
    t = op['fit_tolerance']
    if type(t) not in (int, float) or math.isnan(t) or math.isinf(t) or t <= 0.:
        raise ValueError('invalid surface fit tolerance')


def run(op, host, iterations, tolerance):
    validate(op)
    source = host['_nurbs_surface_from_definition'](op['surface'])
    definition = host['_nurbs_surface_definition']
    try:
        before = definition(source)
        def compute():
            fitted = source.Fit(op['degree_u'], op['degree_v'], op['fit_tolerance'])
            candidate = None
            try:
                candidate = fitted.ToNurbsSurface() if fitted is not None else None
                if candidate is not None and not candidate.IsValid:
                    raise ValueError('invalid surface fit result')
                after = definition(source)
                if after != before:
                    raise ValueError('surface fit mutated its input')
                return dict(surface=definition(candidate) if candidate is not None else None,
                            before=before, after=after)
            finally:
                if candidate is not None and candidate is not fitted:
                    candidate.Dispose()
                if fitted is not None:
                    fitted.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        source.Dispose()
