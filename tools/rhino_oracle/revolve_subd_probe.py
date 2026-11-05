"""Public Revolve SubD control meshes and complete model history."""
if __package__:
    from .revolve_subd_cases import validate
else:
    from revolve_subd_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians);source=S.Guid.Empty
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            g=obj.Geometry;row=dict(source=obj.Id==source,selected=int(obj.IsSelected(False)))
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.SubD):row.update(kind='subd',subd=subd_value(g,host))
            else:raise ValueError('unexpected native revolution geometry')
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable' if op.get('crease_splitting',False) else 'Disable'),False)
        c=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(c)
        finally:c.Dispose()
        if source==S.Guid.Empty:raise ValueError('source insertion failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned SubD revolution '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        p=lambda a:'w'+','.join('%.17g'%x for x in a)
        prefix=' _Output=_Surface _Deformable=_Yes _PointCount=16 _SplitAtTangents=_Yes' if op.get('seed_surface',False) else ''
        macro='_-Revolve %s %s%s _Output=_SubD _SegmentCount=%d _DeleteInput=_%s _AskForStartAngle=_Yes %.17g %.17g'%(p(op['origin']),p(op['axis_end']),prefix,op['segment_count'],'Yes' if op['delete_input'] else 'No',op['start_degrees'],op['sweep_degrees'])
        success,created,events=observe_command(R.Commands.Command,'Revolve',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or not any(r['kind']=='subd' for r in after) or 'Unknown command:' in history:raise ValueError('native SubD revolution failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
