"""Compare public native creator output without changing desktop settings."""
if __package__:
    from .use_extrusions_creators_cases import recipes,validate
else:
    from use_extrusions_creators_cases import recipes,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    case=recipes(True)[op['recipe']];sources=[];saved_plane=vp.GetConstructionPlane()
    original=bool(R.ApplicationSettings.GeneralSettings.UseExtrusions)
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            g=obj.Geometry;row=dict(source=obj.Id in sources,selected=int(obj.IsSelected(False)))
            if isinstance(g,G.Curve):row.update(kind='curve',curve=host['_nurbs_curve_definition'](g))
            else:
                row['kind']='extrusion' if isinstance(g,G.Extrusion) else 'brep'
                brep=g if isinstance(g,G.Brep) else g.ToBrep()
                if brep is None:raise ValueError('missing creator B-rep')
                try:row['geometry']=host['_interchange_brep_record'](brep)
                finally:
                    if brep is not g:brep.Dispose()
                if isinstance(g,G.Extrusion):
                    row['extrusion']=dict(caps=[bool(g.IsCappedAtBottom),bool(g.IsCappedAtTop)],profiles=int(g.ProfileCount),
                        path_start=host['_xyz'](g.PathStart),path_end=host['_xyz'](g.PathEnd),surface=host['_nurbs_surface_definition'](g))
            rows.append(row)
        return rows
    try:
        R.RhinoApp.RunScript('_UseExtrusions _'+op['mode'],False)
        if 'construction_plane' in case:
            origin,x,y=case['construction_plane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        name=case.get('profile');rail=case.get('rail');curve=None
        if rail:
            a,b=G.Point3d(0,0,0),G.Point3d(0,0,5)
            if rail=='reverse':a,b=b,a
            if rail=='oblique':a,b=G.Point3d(1,2,3),G.Point3d(4,6,3)
            curve=G.LineCurve(a,b)
            if rail=='nurbsline':old=curve;curve=old.ToNurbsCurve();old.Dispose()
            if rail=='polyline':old=curve;curve=G.PolylineCurve([a,b]);old.Dispose()
        elif name=='rectangle':curve=G.Rectangle3d(G.Plane.WorldXY,4.,3.).ToNurbsCurve()
        elif name=='circle':curve=G.Circle(G.Plane.WorldXY,2.).ToNurbsCurve()
        elif name=='line':curve=G.LineCurve(G.Point3d(0,0,0),G.Point3d(3,0,0))
        elif name=='nonplanar':curve=G.NurbsCurve.Create(False,3,[G.Point3d(0,0,0),G.Point3d(1,2,1),G.Point3d(2,1,3),G.Point3d(4,0,0)])
        elif name in ('xz','open_kink'):curve=G.PolylineCurve([G.Point3d(0,0,0),G.Point3d(1,0,1) if name=='xz' else G.Point3d(1,1,0),G.Point3d(3,0,0)])
        if curve is not None:
            if 'rail_domain' in case:curve.Domain=G.Interval(*case['rail_domain'])
            try:sources.append(doc.Objects.AddCurve(curve))
            finally:curve.Dispose()
        if name=='surface':
            surface=G.PlaneSurface(G.Plane.WorldXY,G.Interval(0,4),G.Interval(0,3))
            try:sources.append(doc.Objects.AddSurface(surface))
            finally:surface.Dispose()
        for key in sources:doc.Objects.Select(key)
        doc.ClearUndoRecords(True);before=snapshot();marker='Owned extrusion creator '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,created,events=observe_command(R.Commands.Command,case['command'],lambda:R.RhinoApp.RunScript(case['macro'],True),snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or len([row for row in after if not row['source']])<1 or 'Unknown command:' in history:raise ValueError('creator failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,success=success,before=before,created=created,after=after,undo=undo,redo=redo,events=events,history=history),0
    finally:
        R.RhinoApp.RunScript('!',False);vp.SetConstructionPlane(saved_plane)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_UseExtrusions _'+('Extrusion' if original else 'Polysurface'),False)
