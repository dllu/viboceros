"""Public native commands and NURBS views, only in the owned headless document."""
if __package__:
    from .short_span_extrusions_cases import validate
else:
    from short_span_extrusions_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    original=bool(R.ApplicationSettings.GeneralSettings.UseExtrusions);source=S.Guid.Empty
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            g=obj.Geometry;row=dict(source=obj.Id==source,selected=int(obj.IsSelected(False)))
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.Extrusion):
                brep=g.ToBrep()
                try:row.update(kind='extrusion',surface=host['_nurbs_surface_definition'](g),faces=int(brep.Faces.Count),caps=[bool(g.IsCappedAtBottom),bool(g.IsCappedAtTop)])
                finally:brep.Dispose()
            else:raise ValueError('unexpected native short-span output: '+str(g.GetType().Name))
            rows.append(row)
        return rows
    try:
        R.RhinoApp.RunScript('_UseExtrusions _Extrusion',False)
        curve=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(curve)
        finally:curve.Dispose()
        if source==S.Guid.Empty:raise ValueError('source admission failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned short-span extrusion '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-ExtrudeCrv _Output=_Surface _Solid=_No _DeleteInput=_No _BothSides=_'+('Yes'if op['both_sides']else'No')+' '+str(op['height'])
        success,created,events=observe_command(R.Commands.Command,'ExtrudeCrv',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or len(after)!=2 or 'Unknown command:'in history:raise ValueError('native short-span creator failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_UseExtrusions _'+('Extrusion'if original else'Polysurface'),False)
