"""Public SDK signed-length pieces from original NURBS source definitions."""
def run(op,host,iterations):
    _nurbs_curve_from_definition=host['_nurbs_curve_from_definition']
    _nurbs_curve_definition=host['_nurbs_curve_definition']
    _finite=host['_finite']
    _measure=host['_measure']
    _xyz=host['_xyz']
    source = _nurbs_curve_from_definition(op['curve'])
    anchor = _finite(op['start'], 'length subcurve anchor')
    distance = _finite(op['length'], 'length subcurve distance')
    if not source.Domain.IncludesParameter(anchor):
        source.Dispose()
        raise ValueError('length subcurve anchor outside source domain')
    before = _nurbs_curve_definition(source)
    def extract_length_subcurve():
        owned = []
        try:
            oriented = source.DuplicateCurve()
            owned.append(oriented)
            start = anchor
            if distance < 0.:
                if not oriented.Reverse(): raise ValueError('length subcurve reversal failed')
                start = -anchor
            if oriented.IsClosed:
                if not oriented.ChangeClosedCurveSeam(start): raise ValueError('length subcurve seam failed')
                tail = oriented
            elif start == oriented.Domain.T1:
                return dict(available=False,curve=None,source_unchanged=_nurbs_curve_definition(source)==before)
            else:
                tail = oriented.Trim(start,oriented.Domain.T1)
                if tail is None: raise ValueError('length subcurve tail failed')
                owned.append(tail)
            curve = None
            if distance != 0. and abs(distance) <= tail.GetLength():
                ok,end = tail.LengthParameter(abs(distance))
                if not ok: raise ValueError('length subcurve inversion failed')
                result = tail.Trim(tail.Domain.T0,end)
                if result is None: raise ValueError('length subcurve trim failed')
                owned.append(result)
                n = result.ToNurbsCurve()
                owned.append(n)
                curve = dict(definition=_nurbs_curve_definition(n), samples=[_xyz(result.PointAt(result.Domain.ParameterAt(i/32.))) for i in range(33)])
            return dict(available=curve is not None,curve=curve,source_unchanged=_nurbs_curve_definition(source)==before)
        finally:
            for g in reversed(owned): g.Dispose()
    try:
        return _measure(iterations,extract_length_subcurve)
    finally:
        source.Dispose()
