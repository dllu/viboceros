"""Observe the public RhinoCommon compatible-curve API without mutating sources."""


def definitions(curves, geometry, definition, angle_tolerance):
    prepared = list(geometry.NurbsCurve.MakeCompatible(
        curves, geometry.Point3d.Unset, geometry.Point3d.Unset,
        0, 0, 0., angle_tolerance) or [])
    try:
        if len(prepared) != len(curves) or any(not c.IsValid for c in prepared):
            raise ValueError('native compatible curves are incomplete or invalid')
        return [definition(c) for c in prepared]
    finally:
        for curve in reversed(prepared): curve.Dispose()


def preparation(curves, geometry, definition, absolute_tolerance, angle_tolerance):
    records = []
    for curve in curves:
        record = dict(individual=definitions([curve], geometry, definition, angle_tolerance),
                      length=float(curve.GetLength(1e-2)))
        nurbs = curve.ToNurbsCurve()
        try:
            knots = sorted(set(float(t) for t in nurbs.Knots))
            spans = []
            for a, b in zip(knots, knots[1:]):
                part = nurbs.Trim(a, b)
                if part is None: raise ValueError('could not trim compatibility length span')
                try: spans.append(dict(domain=[a,b],length=float(part.GetLength(1e-2))))
                finally: part.Dispose()
            record['spans'] = spans
        finally: nurbs.Dispose()
        simplified = curve.Simplify(geometry.CurveSimplifyOptions.All,
                                    absolute_tolerance, angle_tolerance)
        if simplified is None: simplified = curve.DuplicateCurve()
        try:
            pieces = []
            count = simplified.SegmentCount if isinstance(simplified,geometry.PolyCurve) else 1
            for index in range(count):
                child = simplified.SegmentCurve(index) if isinstance(simplified,geometry.PolyCurve) else simplified
                outer = simplified.SegmentDomain(index) if isinstance(simplified,geometry.PolyCurve) else child.Domain
                child_nurbs = child.ToNurbsCurve()
                try:
                    pieces.append(dict(type=child.GetType().Name,domain=[child.Domain.T0,child.Domain.T1],
                        outer_domain=[outer.T0,outer.T1],length=float(child.GetLength(1e-2)),nurbs=definition(child_nurbs)))
                finally: child_nurbs.Dispose()
            record['simplified'] = dict(type=simplified.GetType().Name,
                domain=[simplified.Domain.T0,simplified.Domain.T1],pieces=pieces)
        finally: simplified.Dispose()
        records.append(record)
    return records


def matching(curves, geometry, definition, continuity=None):
    result=[]
    key='c1_parameters' if continuity is None else 'parameters'
    level=geometry.Continuity.C1_continuous if continuity is None else continuity
    for curve in curves:
        domain=curve.Domain;cursor=domain.T0;cuts=[]
        while True:
            found,t=curve.GetNextDiscontinuity(level,cursor,domain.T1)
            if not found:break
            if not cursor<t<domain.T1:raise ValueError('invalid native C1 discontinuity traversal')
            cuts.append(float(t));cursor=t
        regions=[]
        parameters=[domain.T0]+cuts+[domain.T1]
        for a,b in zip(parameters,parameters[1:]):
            part=curve.Trim(a,b)
            if part is None:raise ValueError('could not trim C1 compatibility region')
            try:regions.append(dict(domain=[a,b],length=float(part.GetLength(1e-3)),nurbs=definition(part)))
            finally:part.Dispose()
        result.append({key:cuts,'regions':regions})
    return result


def speed_samples(curves, include_derivatives=False):
    result=[]
    for curve in curves:
        parameters=[float(curve.Domain.ParameterAt(i/32.))for i in range(33)]
        speeds=[];derivatives=[]
        for t in parameters:
            values=curve.DerivativeAt(t,1)
            if values is None or len(values)!=2:raise ValueError('native curve derivative unavailable')
            speeds.append(float(values[1].Length))
            if include_derivatives:derivatives.append([float(values[1].X),float(values[1].Y),float(values[1].Z)])
        row=dict(parameters=parameters,speeds=speeds)
        if include_derivatives:row['derivatives']=derivatives
        result.append(row)
    return result


def quadrature_samples(curves):
    """Public span queries at the four-step Cash--Karp coarse-rule nodes."""
    fractions=[]
    for start,step in [(0.,.01),(.01,.05),(.06,.25),(.31,.69)]:
        for node in [0.,.2,.3,.6,1.,.875]:
            t=min(1.,start+node*step)
            if t not in fractions:fractions.append(t)
    records=[]
    for curve in curves:
        nurbs=curve.ToNurbsCurve()
        try:
            knots=sorted(set(float(t)for t in nurbs.Knots));spans=[]
            for a,b in zip(knots,knots[1:]):
                part=nurbs.Trim(a,b)
                if part is None:raise ValueError('could not trim quadrature span')
                try:
                    part.Domain=type(part.Domain)(0.,1.)
                    rows=[]
                    for t in fractions:
                        derivative=part.DerivativeAt(t,1)[1]
                        rows.append(dict(parameter=t,derivative=[float(derivative.X),float(derivative.Y),float(derivative.Z)],speed=float(derivative.Length)))
                    spans.append(dict(source_domain=[a,b],length=float(part.GetLength(1e-3)),samples=rows))
                finally:part.Dispose()
            records.append(spans)
        finally:nurbs.Dispose()
    return records
