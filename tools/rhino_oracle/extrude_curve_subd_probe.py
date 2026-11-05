"""Public ExtrudeCrv SubD control meshes and complete model history."""
if __package__:
    from .extrude_curve_subd_cases import validate
else:
    from extrude_curve_subd_cases import validate


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
            else:raise ValueError('unexpected native extrusion geometry')
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        c=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(c)
        finally:c.Dispose()
        if source==S.Guid.Empty:raise ValueError('source insertion failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned SubD extrusion '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        p=lambda a:'w'+','.join('%.17g'%x for x in a)
        yes=lambda b:'Yes' if b else 'No'
        length=sum(x*x for x in op['direction'])**.5
        path=('%.17g'%op['distance']) if 'distance' in op else ('_Direction w0,0,0 %s %.17g'%(p(op['direction']),length))
        macro='_-ExtrudeCrv _Output=_Surface _Solid=_%s _Output=_SubD _BothSides=_%s _DeleteInput=_%s %s'%(yes(op['solid']),yes(op['both_sides']),yes(op['delete_input']),path)
        success,created,events=observe_command(R.Commands.Command,'ExtrudeCrv',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or not any(r['kind']=='subd' for r in after) or 'Unknown command:' in history:raise ValueError('native SubD extrusion failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
