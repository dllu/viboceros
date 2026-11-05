"""Compare the actual Length command with public mesh/SubD edge representations."""
if __package__:
    from .component_length_cases import source,validate
    from .subd_join_probe import source as make_subd
else:
    from component_length_cases import source,validate
    from subd_join_probe import source as make_subd

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty private idle document required')
    recipe=source(op['scene']);mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
    geometry=None;line_id=None;old_display=doc.SubDAppearance
    try:
        doc.ModelUnitSystem=R.UnitSystem.Millimeters
        for p in recipe['points']:mesh.Vertices.Add(G.Point3d(*p))
        for f in recipe['faces']:mesh.Faces.AddFace(*f)
        mesh.Normals.ComputeNormals()
        geometry=mesh.DuplicateMesh() if op['kind']=='mesh' else (G.Brep.CreateFromCornerPoints(*([G.Point3d(*p)for p in recipe['points']]+[doc.ModelAbsoluteTolerance])) if op['kind']=='brep' else make_subd(recipe,host))
        key=doc.Objects.Add(geometry)
        if key==S.Guid.Empty:raise ValueError('cannot add owned edge source')
        if op['curve']:line_id=doc.Objects.AddLine(G.Point3d(30,0,0),G.Point3d(33,0,0))
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
        edges=[];native=[];lookup={tuple(p):i for i,p in enumerate(recipe['points'])}
        xyz=lambda p:(float(p.X),float(p.Y),float(p.Z))
        if op['kind']=='mesh':
            for i in range(geometry.TopologyEdges.Count):
                line=geometry.TopologyEdges.EdgeLine(i)
                edges.append([lookup[xyz(line.From)],lookup[xyz(line.To)]]);native.append(i)
        elif op['kind']=='brep':
            for i,edge in enumerate(geometry.Edges):
                edges.append([lookup[xyz(edge.PointAtStart)],lookup[xyz(edge.PointAtEnd)]]);native.append(i)
        else:
            for edge in geometry.Edges:
                edges.append([lookup[xyz(edge.VertexFrom.ControlNetPoint)],lookup[xyz(edge.VertexTo.ControlNetPoint)]]);native.append(int(edge.Id))
        selected=[next(i for i,e in enumerate(edges)if set(e)==set(pair))for pair in op['pairs']]
        crc=int(geometry.DataCRC(0));doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60',False);R.RhinoApp.RunScript('_Undo',False)
        obj=doc.Objects.FindId(key)
        for i in ([] if op.get('steps') else selected):
            kind=G.ComponentIndexType.MeshTopologyEdge if op['kind']=='mesh' else (G.ComponentIndexType.BrepEdge if op['kind']=='brep' else G.ComponentIndexType.SubdEdge)
            if not obj.SelectSubObject(G.ComponentIndex(kind,native[i]),True,True,False):raise ValueError('cannot select owned edge')
        if line_id is not None and not op.get('steps'):doc.Objects.Select(line_id)
        def snapshot():
            obj=doc.Objects.FindId(key)
            return dict(whole=bool(obj.IsSelected(False)),components=[dict(kind=str(c.ComponentIndexType),index=int(c.Index))for c in (obj.GetSelectedSubObjects()or[])],objects=len(list(doc.Objects)),curve=False if line_id is None else bool(doc.Objects.FindId(line_id).IsSelected(False)))
        before=snapshot();marker='Owned edge length '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        trace=[]
        if op.get('steps'):
            from shrink_face_input import drive
            from chain_selection_driver import DispatchTimer
            R.RhinoApp.RunScript('_SetView _World _Top',False);R.RhinoApp.RunScript('_Zoom _Extents',False)
            doc.Views.ActiveView.ActiveViewport.DisplayMode=R.Display.DisplayModeDescription.FindByName('Wireframe');doc.Views.Redraw()
            a,b=[G.Point3d(*recipe['points'][i])for i in op['pairs'][0]]
            point=G.Point3d((a.X+b.X)*.5,(a.Y+b.Y)*.5,(a.Z+b.Z)*.5)
            if op['kind']=='subd' and not op['flat']:
                geometry.UpdateSurfaceMeshCache(False);c=list(geometry.Edges)[selected[0]].ToNurbsCurve(True)
                if c is None:raise ValueError('cannot locate native limit edge')
                try:point=c.PointAt(c.Domain.T0+(c.Domain.T1-c.Domain.T0)*.5)
                finally:c.Dispose()
            positions=[[G.Point3d(31.5,0,0)if step.get('target')=='curve' else point]for step in op['steps']]
            success=drive(op,positions,host,snapshot,trace,'Length',script='_Length _Pause',timer_factory=DispatchTimer,input_ready=lambda:'_Pause' in R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1] and R.Input.RhinoGet.InGetObject(doc))
        else:success=R.RhinoApp.RunScript('_Length' if op.get('bare') else '_Length _Enter',True)
        if R.Commands.Command.InCommand():raise ValueError('Length left an active getter')
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        curves=[]
        if op['kind']=='subd':
            geometry.UpdateSurfaceMeshCache(False)
            native_edges=list(geometry.Edges)
            for i in selected:
                c=native_edges[i].ToNurbsCurve(True)
                try:curves.append(None if c is None else dict(definition=host['_nurbs_curve_definition'](c),length=c.GetLength()))
                finally:
                    if c is not None:c.Dispose()
        unchanged=int(doc.Objects.FindId(key).Geometry.DataCRC(0))==crc
        if not unchanged:raise ValueError('Length edited an edge source')
        model=R.FileIO.File3dm()
        try:
            if op['kind']=='mesh':model.Objects.AddMesh(geometry)
            elif op['kind']=='brep':model.Objects.AddBrep(geometry)
            else:model.Objects.AddSubD(geometry)
            archive=[int(b)for b in model.ToByteArray()]
        finally:model.Dispose()
        return dict(input=recipe,edges=edges,native=native,selected=selected,archive=archive,curves=curves,before=before,trace=trace,after=after,redo=redo,undo=undo,history=history,success=success,source_unchanged=unchanged),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.SubDAppearance=old_display
        if geometry is not None:geometry.Dispose()
        mesh.Dispose()
