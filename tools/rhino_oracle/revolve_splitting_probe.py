"""Public Revolve topology, complete definitions and history in an owned doc."""
if __package__:
    from .revolve_splitting_cases import validate
else:
    from revolve_splitting_cases import validate

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    source=S.Guid.Empty
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    def record(b):
        types=[str(t.TrimType)for f in b.Faces for l in f.Loops for t in l.Trims]
        return dict(vertices=int(b.Vertices.Count),edges=int(b.Edges.Count),faces=int(b.Faces.Count),solid=bool(b.IsSolid),
            valences=sorted(int(e.Valence)for e in b.Edges),trim_types=sorted(types),
            surfaces=[dict(surface=host['_nurbs_surface_definition'](f.UnderlyingSurface()),reversed=bool(f.OrientationIsReversed),loops=int(f.Loops.Count))for f in b.Faces])
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            g=obj.Geometry;row=dict(source=obj.Id==source,selected=int(obj.IsSelected(False)))
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            else:
                b=g if isinstance(g,G.Brep)else g.ToBrep()
                try:
                    if b is None or not b.IsValid:raise ValueError('invalid native revolution output')
                    row.update(kind='surface',brep=record(b))
                finally:
                    if b is not g and b is not None:b.Dispose()
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        R.RhinoApp.RunScript('_-CreaseSplitting _'+('Enable'if op['crease_splitting']else'Disable'),False)
        c=host['_nurbs_curve_from_definition'](op['curve'])
        try:source=doc.Objects.AddCurve(c)
        finally:c.Dispose()
        if source==S.Guid.Empty:raise ValueError('source insertion failed')
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned revolution splitting '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        p=lambda a:'w'+','.join('%.17g'%x for x in a)
        yes=lambda b:'Yes'if b else'No'
        macro='_-Revolve %s %s _Output=_Surface _Deformable=_%s'%(p(op['origin']),p(op['axis_end']),yes(op['deformable']))
        if op['deformable']:macro+=' _PointCount='+str(op['point_count'])
        macro+=' _SplitAtTangents=_%s _DeleteInput=_%s _AskForStartAngle=_Yes %.17g %.17g'%(yes(op['split_at_tangents']),yes(op['delete_input']),op['start_degrees'],op['sweep_degrees'])
        success,created,events=observe_command(R.Commands.Command,'Revolve',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or not any(not row['source']for row in after)or'Unknown command:'in history:raise ValueError('native revolution failed: '+history)
        full=[]
        for obj in doc.Objects:
            if obj.Id==source:continue
            g=obj.Geometry;b=g if isinstance(g,G.Brep)else g.ToBrep()
            try:full.append(host['_interchange_brep_record'](b,include_samples=True))
            finally:
                if b is not g:b.Dispose()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,full=full,
            document_tolerance=dict(absolute=doc.ModelAbsoluteTolerance,relative=doc.ModelRelativeTolerance,angular=doc.ModelAngleToleranceRadians)),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True);R.RhinoApp.RunScript('_-CreaseSplitting _Enable',False)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
