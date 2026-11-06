"""Observe owned PlanarSrf commands, exact boundaries and chronological history."""
if __package__:
    from .planar_regions_cases import validate
else:
    from planar_regions_cases import validate


def run(op,host,iterations,tolerance,validator=validate,subset=False):
    validator(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    saved=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    curves=[];ids=[]
    def match(a,b):
        epsilon=tolerance['absolute']if subset else 1e-7
        ba,bb=a.GetBoundingBox(True),b.GetBoundingBox(True)
        if not subset and (ba.Min.DistanceTo(bb.Min)>epsilon or ba.Max.DistanceTo(bb.Max)>epsilon):return False
        for first,second in ([(b,a)]if subset else[(a,b),(b,a)]):
            for i in range(33):
                p=first.PointAt(first.Domain.ParameterAt(i/32.));success,t=second.ClosestPoint(p)
                if not success:return False
                for step in range(12):
                    derivatives=second.DerivativeAt(t,2)
                    if derivatives is None or len(derivatives)<3:break
                    q=second.PointAt(t);d=q-p;v=derivatives[1];a=derivatives[2]
                    numerator=d.X*v.X+d.Y*v.Y+d.Z*v.Z
                    denominator=v.SquareLength+d.X*a.X+d.Y*a.Y+d.Z*a.Z
                    if denominator<=0.:break
                    proposal=max(second.Domain.T0,min(second.Domain.T1,t-numerator/denominator))
                    if proposal==t:break
                    t=proposal
                if p.DistanceTo(second.PointAt(t))>epsilon:return False
        return True
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            row=dict(source=ids.index(obj.Id)if obj.Id in ids else None,selected=int(obj.IsSelected(False)))
            g=obj.Geometry
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.Brep):
                loops=[]
                for face in g.Faces:
                    for loop in face.Loops:
                        c=loop.To3dCurve()
                        try:
                            bounds=c.GetBoundingBox(True)
                            loops.append(dict(kind=str(loop.LoopType),sources=[i for i,source in enumerate(curves)if match(c,source)],min=host['_xyz'](bounds.Min),max=host['_xyz'](bounds.Max)))
                        finally:c.Dispose()
                area=G.AreaMassProperties.Compute(g,True,False,False,False,1e-12,1e-12)
                try:value=float(area.Area)if area else None
                finally:
                    if area:area.Dispose()
                row.update(kind='brep',faces=int(g.Faces.Count),loops=loops,area=value,native_valid=bool(g.IsValid),
                           brep=host['_interchange_brep_record'](g,include_samples=False))
                centered=g.DuplicateBrep();origin=g.GetBoundingBox(True).Center
                try:
                    if not centered.Transform(G.Transform.Translation(-origin.X,-origin.Y,-origin.Z)):raise ValueError('area translation failed')
                    area=G.AreaMassProperties.Compute(centered,True,False,False,False,1e-12,1e-12)
                    try:row.update(centered_area=float(area.Area)if area else None,area_origin=host['_xyz'](origin))
                    finally:
                        if area:area.Dispose()
                finally:centered.Dispose()
                unit=g.DuplicateBrep()
                try:
                    for face in unit.Faces:
                        for axis in range(2):
                            if not face.SetDomain(axis,G.Interval(0.,1.)):raise ValueError('unit face domain failed')
                    area=G.AreaMassProperties.Compute(unit,True,False,False,False,1e-12,1e-12)
                    try:row.update(unit_domain_area=float(area.Area)if area else None,
                                   unit_domain_brep=host['_interchange_brep_record'](unit,include_samples=False))
                    finally:
                        if area:area.Dispose()
                finally:unit.Dispose()
                if 'samples'in op:
                    from planar_crossings_probe import details
                    row['coverage']=details(g,op['samples'],host)
            else:raise ValueError('unexpected PlanarSrf output')
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        for definition in op['curves']:
            c=host['_nurbs_curve_from_definition'](definition);curves.append(c);key=doc.Objects.AddCurve(c)
            if key==S.Guid.Empty:raise ValueError('source insertion failed')
            ids.append(key);doc.Objects.Select(key)
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned planar-region probe '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,created,events=observe_command(R.Commands.Command,'PlanarSrf',lambda:R.RhinoApp.RunScript('_-PlanarSrf',True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand()or 'Unknown command:'in history:raise ValueError('PlanarSrf did not return to idle: '+history)
        if any(r['kind']=='brep'for r in after):
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo=before;redo=after
        relationships=[]
        for c in ([]if subset else curves):
            ok,plane=c.TryGetPlane()
            relationships.append([str(c.Contains(other.PointAtStart,plane,tolerance['absolute']))if ok else None for other in curves])
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history,contains_start=relationships),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        for c in curves:c.Dispose()
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved
