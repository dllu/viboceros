"""Public two-view reconstruction without changes outside an owned idle document."""
if __package__:
    from .curve_two_views_cases import validate
else:
    from curve_two_views_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    if op['mode']=='surface_audit':
        from curve_two_views_surface_probe import run as surface_audit
        return surface_audit(op,host,iterations,tolerance)
    if op['mode']=='fit_audit':
        from curve_two_views_fit_probe import run as fit_audit
        return fit_audit(op,host,iterations,tolerance)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    old=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians,doc.Layers.CurrentLayerIndex)
    ids=[];owned=[];layers=[];group=None
    def strings(obj):
        values=obj.GetUserStrings()
        return {str(k):str(values[k])for k in values.AllKeys}if values is not None else{}
    def curve_value(g):
        return dict(curve=host['_nurbs_curve_definition'](g),native_type=g.GetType().Name,closed=bool(g.IsClosed),native_valid=bool(g.IsValid),length=float(g.GetLength()))
    def snapshot():
        objects=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a=obj.Attributes;row=dict(source=ids.index(obj.Id)if obj.Id in ids else None,selected=int(obj.IsSelected(False)),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=[doc.Groups.GroupName(i)for i in (a.GetGroupList()or[])],user_text=strings(a)),geometry_user_text=strings(obj.Geometry))
            row.update(curve_value(obj.Geometry));objects.append(row)
        return dict(objects=objects,group_alive=group is not None and not doc.Groups[group].IsDeleted)
    try:
        doc.ModelAbsoluteTolerance=op.get('model_tolerance',tolerance['absolute']);doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        for name in ['Crv2View Input','Crv2View Output']:
            layer=R.DocObjects.Layer();layer.Name=name;index=doc.Layers.Add(layer);layer.Dispose();layers.append(index)
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        for i,definition in enumerate(op['curves']):
            curve=host['_nurbs_curve_from_definition'](definition);curve.SetUserString('geometry-origin','source-'+str(i));owned.append(curve)
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='source-'+str(i);a.ObjectColor=S.Drawing.Color.FromArgb(25+i,67+i,109+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.SetUserString('attribute-origin','source-'+str(i))
            key=doc.Objects.AddCurve(curve,a);a.Dispose()
            if key==S.Guid.Empty:raise ValueError('source insertion failed')
            ids.append(key)
        group=doc.Groups.Add('Crv2View Sources',ids)
        if op.get('preselect'):
            for key in ids:doc.Objects.Select(key)
        doc.ClearUndoRecords(True);before=snapshot()
        if op['mode']in ['sdk','audit']:
            checks=[int(c.DataCRC(0))for c in owned]
            result=G.Curve.CreateCurve2View(owned[0],owned[1],G.Vector3d(*op['directions'][0]),G.Vector3d(*op['directions'][1]),op.get('api_tolerance',tolerance['absolute']),op.get('api_angle',tolerance['angular']))
            audit=None
            try:
                output=[curve_value(c)for c in(result or[])]
                if op['mode']=='audit':
                    expected=G.NurbsCurve(3,False,4,4)
                    try:
                        for i in range(4):
                            a,b=owned[0].Points[i].Location,owned[1].Points[i].Location
                            expected.Points.SetPoint(i,G.Point3d(a.X,a.Y,b.Z))
                        for i,knot in enumerate([0.,0.,0.,1.,1.,1.]):expected.Knots[i]=knot
                        maximum=0.;worst=None;source_max=[0.,0.];known_max=0.
                        for curve in(result or[]):
                            for span in range(curve.SpanCount):
                                interval=curve.SpanDomain(span)
                                for j in range(9):
                                    point=curve.PointAt(interval.ParameterAt(j/8.));lo,hi=0.,1.
                                    for step in range(70):
                                        mid=(lo+hi)*.5
                                        if expected.PointAt(mid).X<point.X:lo=mid
                                        else:hi=mid
                                    truth=expected.PointAt((lo+hi)*.5);distance=point.DistanceTo(truth)
                                    if distance>maximum:maximum=distance;worst=dict(output=host['_xyz'](point),same_x_expected=host['_xyz'](truth),distance=distance)
                                    ok,t=expected.ClosestPoint(point)
                                    if not ok:raise ValueError('known curve closest point failed')
                                    known_max=max(known_max,point.DistanceTo(expected.PointAt(t)))
                                    for i,projection in enumerate([G.Point3d(point.X,point.Y,0.),G.Point3d(point.X,0.,point.Z)]):
                                        ok,t=owned[i].ClosestPoint(projection)
                                        if not ok:raise ValueError('profile closest point failed')
                                        source_max[i]=max(source_max[i],projection.DistanceTo(owned[i].PointAt(t)))
                        audit=dict(maximum_same_x_distance=maximum,maximum_known_closest_distance=known_max,maximum_profile_closest_distances=source_max,worst=worst,samples_per_span=9)
                    finally:expected.Dispose()
            finally:
                for c in(result or[]):c.Dispose()
            if checks!=[int(c.DataCRC(0))for c in owned]or before!=snapshot():raise ValueError('SDK input changed')
            return dict(output=output,before=before,after=snapshot(),success=bool(output),audit=audit),0
        macro='_-Crv2View'
        for i,key in enumerate(ids)if op['mode']!='preselected'else []:
            direction=' _Direction w0,0,0 w'+','.join(format(v,'.17g')for v in op['directions'][i])
            if op['mode']=='before':macro+=direction
            interval=op.get('ranges',[None,None])[i]
            if interval is None:macro+=' _SelID '+str(key)
            else:macro+=' _SubCrv _SelID '+str(key)+' '+' '.join('w'+','.join(format(x,'.17g')for x in p)for p in interval)
            if op['mode']=='after':macro+=direction
        if op.get('cancel'):
            phase=op['cancel'];macro='_-Crv2View'
            if phase.endswith('second'):macro+=' _Direction w0,0,0 w0,0,1 _SelID '+str(ids[0])
            if phase.startswith('direction_'):
                macro+=' _Direction'
                if phase.startswith('direction_end'):macro+=' w0,0,0'
            macro+=' _Cancel _Cancel'
        macro+=' _Enter _Cancel _Cancel _Cancel'
        marker='Owned Crv2View '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,created,events=observe_command(R.Commands.Command,'Crv2View',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('Crv2View did not return to idle: '+history)
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo=before;redo=after
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        for curve in owned:curve.Dispose()
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[:3]
        doc.Layers.SetCurrentLayerIndex(old[3],True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
