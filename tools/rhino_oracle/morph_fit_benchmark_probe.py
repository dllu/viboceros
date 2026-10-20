"""Public Stretch SDK timings, excluding duplication, fixtures and readback."""
try:
    from . import twist_command_probe as source
except (ImportError, ValueError):
    import twist_command_probe as source

AXES = {'Reverse': ([0., 0., 10.], [0., 0., 0.]),
        'Spatial': ([1., 2., 3.], [5., 6., 11.]),
        'Skew': ([1., 2., 3.], [4., -2., 11.])}


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'shape', 'axis'} or
            op['op'] != 'morph_fit_benchmark' or op['shape'] not in ('Curve', 'Surface', 'Box') or
            op['axis'] not in AXES or op['id'] != 'axis_'+op['axis']+'_'+op['shape']):
        raise ValueError('invalid bounded morph benchmark')


def run(op, host, iterations, tolerance):
    validate(op)
    if not 1 <= iterations <= 20:
        raise ValueError('one to twenty timed morphs required')
    R, S = host['Rhino'], host['System']
    owned = source.source_geometry(op['shape'], host)
    saved = R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance
    a, b = [host['_point'](p) for p in AXES[op['axis']]]
    morph = R.Geometry.Morphs.StretchSpaceMorph(a, b, a.DistanceTo(b)*.5)
    morph.Tolerance = 1e-6
    last = None
    samples = []
    try:
        R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance = 1e-6
        before = [source.geometry_record(g, host) for g in owned]
        for step in range(iterations+1):
            current = [g.Duplicate() for g in owned]
            try:
                start = S.Diagnostics.Stopwatch.GetTimestamp()
                statuses = [bool(morph.Morph(g)) for g in current]
                elapsed = S.Diagnostics.Stopwatch.GetTimestamp()-start
                if not all(statuses):
                    raise ValueError('native fitting failed')
                if step:
                    samples.append(int(round(float(elapsed)*1e9/S.Diagnostics.Stopwatch.Frequency)))
                if last is not None:
                    for g in last:
                        g.Dispose()
                last, current = current, None
            finally:
                if current is not None:
                    for g in current:
                        g.Dispose()
        after = [source.geometry_record(g, host) for g in last]
        ordered = sorted(samples)
        return dict(source=before, result=after, times_ns=samples,
                    minimum_ns=ordered[0], median_ns=ordered[len(ordered)//2],
                    fit_tolerance=float(morph.Tolerance)), sum(samples)//iterations
    finally:
        for g in (last or [])+owned:
            g.Dispose()
        morph.Dispose()
        R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance = saved
