"""Observe public CurveBoolean commands and complete object/history state."""
if __package__:
    from .curve_boolean_cases import validate
else:
    from curve_boolean_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    old=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians,doc.Layers.CurrentLayerIndex)
    viewport=doc.Views.ActiveView.ActiveViewport;old_plane=viewport.GetConstructionPlane();layers=[];ids=[];curves=[];group=None
    def strings(obj):
        values=obj.GetUserStrings()
        return {str(k):str(values[k])for k in values.AllKeys}if values is not None else{}
    def snapshot():
        objects=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a=obj.Attributes;g=obj.Geometry
            row=dict(source=ids.index(obj.Id)if obj.Id in ids else None,selected=int(obj.IsSelected(False)),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                groups=[doc.Groups.GroupName(i)for i in (a.GetGroupList()or[])],user_text=strings(a)),geometry_user_text=strings(g))
            if isinstance(g,G.Curve):
                row.update(kind='curve',curve=host['_nurbs_curve_definition'](g),native_type=g.GetType().Name,closed=bool(g.IsClosed),native_valid=bool(g.IsValid))
                if g.IsClosed:
                    area=G.AreaMassProperties.Compute(g)
                    try:row['area']=float(area.Area)if area else None
                    finally:
                        if area:area.Dispose()
            elif isinstance(g,G.Brep):
                area=G.AreaMassProperties.Compute(g,True,False,False,False,1e-12,1e-12)
                try:row.update(kind='brep',brep=host['_interchange_brep_record'](g,include_samples=False),area=float(area.Area)if area else None,native_valid=bool(g.IsValid))
                finally:
                    if area:area.Dispose()
            else:raise ValueError('unexpected CurveBoolean geometry')
            objects.append(row)
        return dict(objects=objects,group_alive=group is not None and not doc.Groups[group].IsDeleted)
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular'];viewport.SetConstructionPlane(G.Plane.WorldXY)
        if op.get('cplane'):
            p=op['cplane'];viewport.SetConstructionPlane(G.Plane(G.Point3d(*p['origin']),G.Vector3d(*p['x_axis']),G.Vector3d(*p['y_axis'])))
        for name in ['CurveBoolean Input','CurveBoolean Output']:
            layer=R.DocObjects.Layer();layer.Name=name;index=doc.Layers.Add(layer);layer.Dispose();layers.append(index)
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        definitions=[('curve',d)for d in op['curves']]+[('surface',d)for d in op.get('surfaces',[])]
        for i,(kind,definition)in enumerate(definitions):
            if kind=='curve':curve=host['_nurbs_curve_from_definition'](definition)
            else:
                surface=host['_nurbs_surface_from_definition'](definition);curves.append(surface);curve=surface.ToBrep()
            curve.SetUserString('geometry-origin','source-'+str(i));curves.append(curve)
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='source-'+str(i);a.ObjectColor=S.Drawing.Color.FromArgb(25+i,67+i,109+i);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.SetUserString('attribute-origin','source-'+str(i))
            key=doc.Objects.AddCurve(curve,a)if kind=='curve'else doc.Objects.AddBrep(curve,a);a.Dispose()
            if key==S.Guid.Empty:raise ValueError('source insertion failed')
            ids.append(key)
        group=doc.Groups.Add('CurveBoolean Sources',ids)
        if op.get('preselect',True):
            for key in ids:doc.Objects.Select(key)
        doc.ClearUndoRecords(True);before=snapshot()
        options=op['options'];macro='_-CurveBoolean'
        if op.get('cancel_subcurve'):
            phase=op['cancel_subcurve'];macro+=' _SubCrv'
            if phase!='source':macro+=' _SelID '+str(ids[op['selections'][0]['source']])
            if phase=='end':macro+=' w'+','.join(format(x,'.17g')for x in op['selections'][0]['range'][0])
            macro+=' _Cancel _Cancel'
        elif op.get('selections')is not None:
            for selection in op['selections']:
                key=ids[selection['source']]
                if selection['range']is None:macro+=' _SelID '+str(key)
                else:
                    macro+=' _SubCrv _Pause'if 'edge'in selection else' _SubCrv _SelID '+str(key)
                    macro+=' w'+','.join(format(x,'.17g')for x in selection['range'][0])
                    if 'length'in selection:macro+=' '+format(selection['length'],'.17g')
                    if selection.get('confirmation',True):macro+=' w'+','.join(format(x,'.17g')for x in selection['range'][1])
            macro+=' _Enter'
        elif not op.get('preselect',True):macro+=' '+' '.join('_SelID '+str(key)for key in ids)+' _Enter'
        if options and not op.get('cancel_subcurve'):
            macro+=' _Output=_Curves _SimplifyCurves=_'+('Yes'if options['simplify']else'No')
            macro+=' _CombineRegions=_'+('Yes'if options['combine']else'No')+' _DeleteInput=_'+options['delete_input']+' _Output=_'+options['output']
        if op.get('cancel_subcurve'):pass
        elif op['all_regions']:macro+=' _AllRegions'
        else:
            for p in op['points']:macro+=' w'+','.join(format(x,'.17g')for x in p)
        if not op.get('cancel_subcurve'):macro+=' _Cancel'if op.get('cancel')else' _Enter'
        marker='Owned CurveBoolean '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        cancel_timer=None;cancel_guard=[];cancel_errors=[];edge_guard=[]
        edge_targets=[s for s in op.get('selections',[])if 'edge'in s]
        if op.get('cancel_subcurve'):
            import Eto.Forms as Forms
            cancel_timer=Forms.UITimer();cancel_timer.Interval=.2
            cancel_started=S.DateTime.UtcNow
            def finish_cancel(sender,args):
                elapsed=(S.DateTime.UtcNow-cancel_started).TotalSeconds
                prompt=str(R.RhinoApp.CommandPrompt)
                if elapsed<1. or(cancel_guard and elapsed-cancel_guard[-1]['elapsed']<1.):return
                if R.Commands.Command.InCommand()and prompt.lower().startswith('select curve'):
                    cancel_timer.Stop();cancel_guard.append(dict(prompt=prompt,elapsed=elapsed,action='CancelRemainingCommand'))
                    R.RhinoApp.RunScript('!',False)
                elif elapsed>15.:
                    if not cancel_errors:cancel_errors.append('nested cancellation did not return to source selection')
                    cancel_timer.Stop();R.RhinoApp.RunScript('!',False)
            cancel_timer.Elapsed+=finish_cancel;cancel_timer.Start()
        try:
            def invoke():
                if not edge_targets:return R.RhinoApp.RunScript(macro,True)
                if len(edge_targets)!=1:raise ValueError('one owned physical edge pick required')
                from curve_boolean_edge_input import drive
                returned,guard=drive(doc,ids,edge_targets[0],host,macro,op['id']);edge_guard.append(guard)
                return returned
            success,created,events=observe_command(R.Commands.Command,'CurveBoolean',invoke,snapshot,lambda:[],True)
        finally:
            if cancel_timer:cancel_timer.Stop();cancel_timer.Elapsed-=finish_cancel;cancel_timer.Dispose()
        if cancel_errors:raise ValueError(cancel_errors[0])
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand()or 'Unknown command:'in history:raise ValueError('CurveBoolean did not return to idle: '+history)
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo=before;redo=after
        result=dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro)
        if edge_targets:
            if len(edge_guard)!=1:raise ValueError('owned edge selection was not observed')
            result['edge_guard']=edge_guard
        if op.get('cancel_subcurve'):
            if not cancel_guard:raise ValueError('the cancellation control was not observed')
            result['cancel_guard']=cancel_guard
        if op.get('check_preferences'):
            for key in ids:doc.Objects.Select(key)
            preference_marker='Owned CurveBoolean preferences '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(preference_marker)
            R.RhinoApp.RunScript('_-CurveBoolean _Cancel',True)
            result['preference_history']=R.RhinoApp.CommandHistoryWindowText.split(preference_marker,1)[1].strip()
            if R.Commands.Command.InCommand():raise ValueError('preference control did not return to idle')
        return result,0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        for curve in curves:curve.Dispose()
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[:3]
        doc.Layers.SetCurrentLayerIndex(old[3],True);viewport.SetConstructionPlane(old_plane)
        for index in reversed(layers):doc.Layers.Delete(index,True)
