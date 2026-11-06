"""Independent public Fit probes for the measured profile-preparation floor."""
def run(op,host,iterations,tolerance):
    R,G=host['Rhino'],host['Rhino'].Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    source=[];output=[]
    try:
        source=[host['_nurbs_curve_from_definition'](c)for c in op['curves']];crc=[int(c.DataCRC(0))for c in source]
        output=list(G.Curve.CreateCurve2View(source[0],source[1],G.Vector3d(*op['directions'][0]),G.Vector3d(*op['directions'][1]),tolerance['absolute'],tolerance['angular'])or[])
        if len(output)!=1:raise ValueError('one owned reconstruction required')
        projections=[[],[]];c=output[0]
        for span in range(c.SpanCount):
            interval=c.SpanDomain(span)
            for j in range(9):
                p=c.PointAt(interval.ParameterAt(j/8.));projections[0].append(G.Point3d(p.X,p.Y,0.));projections[1].append(G.Point3d(p.X,0.,p.Z))
        candidates=[]
        for fit_tolerance in sorted(set([1e-4]+[op['scale']*f for f in [1e-6,1e-5,3e-5,1e-4,3e-4,1e-3,1e-2]])):
            fitted=[]
            try:
                fitted=[c.Fit(3,fit_tolerance,tolerance['angular'])for c in source]
                if any(c is None for c in fitted):candidates.append(dict(tolerance=fit_tolerance,failed=True));continue
                errors=[]
                for curve,points in zip(fitted,projections):
                    maximum=0.
                    for p in points:
                        ok,t=curve.ClosestPoint(p)
                        if not ok:raise ValueError('fitted profile closest point failed')
                        maximum=max(maximum,p.DistanceTo(curve.PointAt(t)))
                    errors.append(maximum)
                candidates.append(dict(tolerance=fit_tolerance,maximum_projection_distances=errors,curves=[host['_nurbs_curve_definition'](c)for c in fitted]))
            finally:
                for c in fitted:
                    if c:c.Dispose()
        if crc!=[int(c.DataCRC(0))for c in source]:raise ValueError('input changed')
        return dict(output=[host['_nurbs_curve_definition'](c)for c in output],candidates=candidates,input_unchanged=True,samples_per_span=9),0
    finally:
        for c in output+source:c.Dispose()
