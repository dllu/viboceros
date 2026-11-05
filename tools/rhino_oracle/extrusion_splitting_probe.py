"""Public straight extrusion storage, splitting, caps and complete history."""
if __package__:
    from .extrusion_splitting_cases import validate
else:
    from extrusion_splitting_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians);source=S.Guid.Empty
    original=bool(R.ApplicationSettings.GeneralSettings.UseExtrusions);seed_record=None
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            g=obj.Geometry;row=dict(source=obj.Id==source,selected=int(obj.IsSelected(False)))
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.SubD):row.update(kind='subd',subd=subd_value(g,host))
            elif isinstance(g,(G.Brep,G.Extrusion)):
                b=g if isinstance(g,G.Brep)else g.ToBrep()
                try:
                    row.update(kind='extrusion'if isinstance(g,G.Extrusion)else'brep',solid=bool(b.IsSolid),surfaces=[host['_nurbs_surface_definition'](f.ToNurbsSurface())for f in b.Faces])
                    row['brep']=host['_interchange_brep_record'](b,include_samples=False)
                    row['orientation']=str(b.SolidOrientation)
                finally:
                    if b is not g:b.Dispose()
            else:row.update(kind='other',runtime_type=str(g.GetType().FullName))
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable' if op.get('crease_splitting',True) else 'Disable'),False)
        if 'seed_split'in op:
            R.RhinoApp.RunScript('_UseExtrusions _Polysurface',False)
            c=host['_nurbs_curve_from_definition'](op['curve'])
            try:key=doc.Objects.AddCurve(c)
            finally:c.Dispose()
            doc.Objects.Select(key)
            marker='Owned split preference seed '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            macro='_-ExtrudeCrv _Output=_Surface _Solid=_No _BothSides=_No _DeleteInput=_Yes _SplitAtTangents=_'+('Yes'if op['seed_split']else'No')
            macro+=' !'if op.get('seed_cancel',False)else' _Direction w0,0,0 w0,3,0 3'
            seeded=R.RhinoApp.RunScript(macro,True)
            seed_record=dict(macro=macro,success=seeded,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip(),objects=int(doc.Objects.Count),in_command=int(R.Commands.Command.InCommand()))
            if R.Commands.Command.InCommand():raise ValueError('split seed did not return to idle')
            if op.get('seed_cancel',False):
                if int(doc.Objects.Count)!=1:raise ValueError('cancelled seed changed geometry')
            elif not seeded:raise ValueError('split seed failed')
            for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
            doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_UseExtrusions _'+('Extrusion'if op['use_extrusions']else'Polysurface'),False)
        c=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(c)
        finally:c.Dispose()
        if source==S.Guid.Empty:raise ValueError('source insertion failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned straight extrusion splitting '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        p=lambda a:'w'+','.join('%.17g'%x for x in a)
        yes=lambda b:'Yes' if b else 'No'
        split=''if op['split_at_tangents']is None else' _SplitAtTangents=_%s'%yes(op['split_at_tangents'])
        if op.get('seed_subd',False):split=(' _SplitAtTangents=_Yes'if not op['use_extrusions']else'')+' _Output=_SubD _Output=_Surface'
        length=sum(x*x for x in op['direction'])**.5
        path='_Direction w0,0,0 %s %.17g'%(p(op['direction']),length)
        macro='_-ExtrudeCrv _Output=_Surface _Solid=_%s%s _BothSides=_%s _DeleteInput=_%s %s'%(yes(op['solid']),split,yes(op['both_sides']),yes(op['delete_input']),path)
        success,created,events=observe_command(R.Commands.Command,'ExtrudeCrv',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or 'Unknown command:' in history:raise ValueError('native straight extrusion failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro,seed=seed_record),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        R.RhinoApp.RunScript('_UseExtrusions _'+('Extrusion'if original else'Polysurface'),False)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
