"""Public ExtrudeCrvToPoint surfaces, splitting, caps and complete model history."""
if __package__:
    from .apex_splitting_cases import validate
else:
    from apex_splitting_cases import validate


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
            elif isinstance(g,G.Brep):
                row.update(kind='brep',solid=bool(g.IsSolid),surfaces=[host['_nurbs_surface_definition'](s.ToNurbsSurface()) for s in g.Surfaces])
                row['brep']=host['_interchange_brep_record'](g,include_samples=False)
                row['orientation']=str(g.SolidOrientation)
            elif isinstance(g,G.Surface):row.update(kind='surface',surface=host['_nurbs_surface_definition'](g.ToNurbsSurface()))
            else:row.update(kind='other',runtime_type=str(g.GetType().FullName))
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable' if op.get('crease_splitting',True) else 'Disable'),False)
        c=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(c)
        finally:c.Dispose()
        if source==S.Guid.Empty:raise ValueError('source insertion failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned surface apex extrusion '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        p=lambda a:'w'+','.join('%.17g'%x for x in a)
        yes=lambda b:'Yes' if b else 'No'
        split=(' _SplitAtTangents=_%s'%yes(op['split_at_tangents']))if op.get('set_split',True)else''
        if op.get('seed_subd',False):split=' _SplitAtTangents=_Yes _Output=_SubD _Output=_Surface'
        macro='_-ExtrudeCrvToPoint _Output=_Surface _Solid=_%s%s _DeleteInput=_%s %s'%(yes(op['solid']),split,yes(op['delete_input']),p(op['apex']))
        success,created,events=observe_command(R.Commands.Command,'ExtrudeCrvToPoint',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or 'Unknown command:' in history:raise ValueError('native surface extrusion failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
