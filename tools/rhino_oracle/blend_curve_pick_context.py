"""Passive public object-pick receipts, measured before BlendCrv runs."""
def measure(source,vp,view,pixel,host,object_id):
    R,S=host['Rhino'],host['System']
    point=S.Drawing.Point(int(pixel.X),int(pixel.Y));transform=vp.GetPickTransform(point)
    context=R.Input.Custom.PickContext();curve=source.ToNurbsCurve()
    try:
        context.View=view;context.PickStyle=R.Input.Custom.PickStyle.PointPick;context.PickMode=R.Input.Custom.PickMode.Wireframe
        context.PickLine=vp.ClientToWorld(point);context.SetPickTransform(transform);context.UpdateClippingPlanes()
        ok,t,depth,distance=context.PickFrustumTest(curve)
        references=[]
        for reference in R.RhinoDoc.ActiveDoc.Objects.PickObjects(context):
            try:
                if reference.ObjectId==object_id:
                    selected,parameter=reference.CurveParameter()
                    references.append(dict(selection_method=int(reference.SelectionMethod()),parameter=float(parameter),point=host['_xyz'](selected.PointAt(parameter))if selected is not None else None))
            finally:reference.Dispose()
        return dict(success=bool(ok),parameter=float(t),point=host['_xyz'](curve.PointAt(t))if ok else None,depth=float(depth),distance=float(distance),references=references,
                    transform=[[float(transform[i,j])for j in range(4)]for i in range(4)],domain=[curve.Domain.T0,curve.Domain.T1],mode=str(context.PickMode),style=str(context.PickStyle))
    finally:context.Dispose();curve.Dispose()
