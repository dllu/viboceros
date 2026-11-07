"""Passive public edge-pick and supporting-face input receipts."""
def measure(source,vp,view,pixel,host,object_id):
    R,S=host['Rhino'],host['System'];owned=None
    if isinstance(source,R.Geometry.Surface):owned=source.ToBrep();source=owned
    context=R.Input.Custom.PickContext()
    try:
        point=S.Drawing.Point(int(pixel.X),int(pixel.Y));transform=vp.GetPickTransform(point)
        context.View=view;context.PickStyle=R.Input.Custom.PickStyle.PointPick;context.PickMode=R.Input.Custom.PickMode.Wireframe
        context.SubObjectSelectionEnabled=True;context.PickLine=vp.ClientToWorld(point);context.SetPickTransform(transform);context.UpdateClippingPlanes()
        candidates=[];topology=[]
        for edge in source.Edges:
            topology.append(dict(edge=int(edge.EdgeIndex),domain=[float(edge.Domain.T0),float(edge.Domain.T1)],start=host['_xyz'](edge.PointAtStart),end=host['_xyz'](edge.PointAtEnd)))
            curve=edge.ToNurbsCurve()
            try:ok,t,depth,distance=context.PickFrustumTest(curve)
            finally:curve.Dispose()
            if not ok:continue
            p=edge.PointAt(t);faces=[]
            for index in edge.AdjacentFaces():
                face=source.Faces[index];found,u,v=face.ClosestPoint(p)
                row=dict(face=int(index),reversed=bool(face.OrientationIsReversed),found=bool(found))
                if found:
                    row.update(uv=[float(u),float(v)],point=host['_xyz'](face.PointAt(u,v)),normal=host['_xyz'](face.NormalAt(u,v)),trims=[dict(reversed=bool(trim.IsReversed()),type=str(trim.TrimType))for loop in face.Loops for trim in loop.Trims if trim.Edge is not None and trim.Edge.EdgeIndex==edge.EdgeIndex])
                    evaluated,position,partials=face.Evaluate(u,v,3)
                    row['third_order']=dict(success=bool(evaluated),point=host['_xyz'](position),partials=[host['_xyz'](d)for d in partials]if partials is not None else None)
                    evaluated,position,partials=face.Evaluate(u,v,4)
                    row['fourth_order']=dict(success=bool(evaluated),point=host['_xyz'](position),partials=[host['_xyz'](d)for d in partials]if partials is not None else None)
                faces.append(row)
            candidates.append(dict(edge=int(edge.EdgeIndex),parameter=float(t),point=host['_xyz'](p),tangent=host['_xyz'](edge.TangentAt(t)),depth=float(depth),distance=float(distance),faces=faces))
        references=[]
        for reference in R.RhinoDoc.ActiveDoc.Objects.PickObjects(context):
            try:
                if reference.ObjectId!=object_id:continue
                edge=reference.Edge();curve,t=reference.CurveParameter()
                references.append(dict(edge=int(edge.EdgeIndex)if edge is not None else None,selection_method=int(reference.SelectionMethod()),parameter=float(t),point=host['_xyz'](curve.PointAt(t))if curve is not None else None,component_type=str(reference.GeometryComponentIndex.ComponentIndexType),component=int(reference.GeometryComponentIndex.Index)))
            finally:reference.Dispose()
        return dict(candidates=candidates,references=references,topology=topology,transform=[[float(transform[i,j])for j in range(4)]for i in range(4)],subobjects=True)
    finally:
        context.Dispose()
        if owned is not None:owned.Dispose()
