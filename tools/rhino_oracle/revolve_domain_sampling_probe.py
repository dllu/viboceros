"""Public SDK stations and one-direction rebuild for domain diagnosis."""
if __package__:
    from .revolve_parameterization_probe import validate as base_validate
    from .revolve_deformable_cases import cases
else:
    from revolve_parameterization_probe import validate as base_validate
    from revolve_deformable_cases import cases

def validate(op):
    if not isinstance(op,dict) or op.get('op')!='revolve_domain_sampling':raise ValueError('fixed revolution sampling query required')
    q=dict(op,op='revolve_parameterization');base_validate(q)

def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects):raise ValueError('empty owned document required')
    case=next(row for row in cases() if row['id']==op['case']+'_8_360')
    curve=host['_nurbs_curve_from_definition'](case['curve'])
    axis=G.Line(G.Point3d(*case['origin']),G.Point3d(*case['axis_end']))
    def view(c):
        rev=G.RevSurface.Create(c,axis,0.,6.283185307179586)
        try:
            initial_size=list(rev.GetSurfaceSize())
            lengths=[dict(tolerance=t,length=float(c.GetLength(t)))for t in [1.,.1,.01,.001,.0001,.00001,.000001,.00000001,.0000000001]]
            automatic=c.Reparameterize()
            try:auto_profile=host['_nurbs_curve_definition'](automatic) if automatic is not None else None
            finally:
                if automatic is not None:automatic.Dispose()
            stations=[c.PointAt(c.Domain.ParameterAt(i/64.))for i in range(65)]
            distances=[a.DistanceTo(b)for a,b in zip(stations,stations[1:])]
            surface_stations=[host['_xyz'](rev.PointAt(rev.Domain(0).T0,rev.Domain(1).ParameterAt(i/64.)))for i in range(65)]
            rebuilt=rev.RebuildOneDirection(0,8,G.LoftType.Normal,1e-6)
            try:
                return dict(size=list(rev.GetSurfaceSize()),length=float(c.GetLength()),initial_size=initial_size,
                    distances=distances,surface_stations=surface_stations,lengths=lengths,automatic=auto_profile,
                    samples=[host['_xyz'](c.PointAt(c.Domain.ParameterAt(i/64.)))for i in range(65)],
                    curve=host['_nurbs_curve_definition'](c),
                    rebuilt=host['_nurbs_surface_definition'](rebuilt) if rebuilt is not None else None)
            finally:
                if rebuilt is not None:rebuilt.Dispose()
        finally:rev.Dispose()
    try:
        original=view(curve)
        curve.Transform(G.Transform.Rotation(0.5235987755982988,axis.Direction,axis.From))
        rotated=view(curve)
        return dict(original=original,rotated=rotated),0
    finally:curve.Dispose()
