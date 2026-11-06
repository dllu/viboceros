"""Direct public surface intersections distinguish reconstruction preparation."""
def run(op,host,iterations,tolerance):
    R,G=host['Rhino'],host['Rhino'].Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    source=[];surfaces=[];native=[];sections=[]
    try:
        source=[host['_nurbs_curve_from_definition'](c)for c in op['curves']]
        for c,d in zip(source,op['directions']):
            v=G.Vector3d(*[x*op['extent']for x in d]);s=G.Surface.CreateExtrusion(c,v)
            if s is None:raise ValueError('owned extrusion failed')
            s.Translate(G.Vector3d(-v.X*.5,-v.Y*.5,-v.Z*.5));surfaces.append(s)
        native=list(G.Curve.CreateCurve2View(source[0],source[1],G.Vector3d(*op['directions'][0]),G.Vector3d(*op['directions'][1]),tolerance['absolute'],tolerance['angular'])or[])
        ok,sections,points=G.Intersect.Intersection.SurfaceSurface(surfaces[0],surfaces[1],tolerance['absolute']);sections=list(sections or[])
        if not ok or len(native)!=1 or len(sections)!=1:raise ValueError('one native reconstruction and intersection required')
        errors=[];profile_max=[0.,0.]
        for a,b in [(native[0],sections[0]),(sections[0],native[0])]:
            maximum=0.
            for span in range(a.SpanCount):
                interval=a.SpanDomain(span)
                for j in range(9):
                    p=a.PointAt(interval.ParameterAt(j/8.));success,t=b.ClosestPoint(p)
                    if not success:raise ValueError('native cross-comparison failed')
                    maximum=max(maximum,p.DistanceTo(b.PointAt(t)))
                    if a==sections[0]:
                        for i,projection in enumerate([G.Point3d(p.X,p.Y,0.),G.Point3d(p.X,0.,p.Z)]):
                            success,t=source[i].ClosestPoint(projection)
                            if not success:raise ValueError('native profile query failed')
                            profile_max[i]=max(profile_max[i],projection.DistanceTo(source[i].PointAt(t)))
            errors.append(maximum)
        return dict(success=True,section_curves=[host['_nurbs_curve_definition'](c)for c in sections],reference_curves=[host['_nurbs_curve_definition'](c)for c in native],maximum_bidirectional_sample_distance=max(errors),maximum_section_profile_distances=profile_max,samples_per_span=9),0
    finally:
        for c in sections+native+surfaces+source:c.Dispose()
