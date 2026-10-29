"""Actual MergeFaces options, component/owner selection, ordering and history."""
try:
    from .merge_faces_command_cases import CASES,EXTRA_CASES,ORDER_CASES,validate
except (ImportError,ValueError):
    from merge_faces_command_cases import CASES,EXTRA_CASES,ORDER_CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from merge_faces_command_input import run as drive
    from subd_box_probe import subd_value
    from subd_edit_scene import Scene
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    case=CASES.get(op['case'],EXTRA_CASES.get(op['case'],ORDER_CASES.get(op['case'])));keys=[];layer=group=-1
    def geometry_value(g):
        if isinstance(g,G.Mesh):
            v=host['_polygon_mesh_value'](g)
            v.update(ngons=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in g.Ngons],
                colors=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in g.VertexColors],
                uv=[[float(p.X),float(p.Y)]for p in g.TextureCoordinates],
                normals=[[float(p.X),float(p.Y),float(p.Z)]for p in g.Normals],valid=bool(g.IsValid))
            return 'mesh',v
        if isinstance(g,G.SubD):
            v=subd_value(g,host)
            v.update(vertex_ids=[int(vertex.Id)for vertex in Scene.vertices(g)],edge_ids=[int(e.Id)for e in g.Edges],face_ids=[int(f.Id)for f in g.Faces])
            return 'subd',v
        return 'point',host['_xyz'](g.Location)
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            kind,value=geometry_value(obj.Geometry);a=obj.Attributes
            rows.append(dict(source=keys.index(obj.Id)if obj.Id in keys else None,name=a.Name,kind=kind,geometry=value,
                object_id=str(obj.Id),runtime_serial=int(obj.RuntimeSerialNumber),
                layer=doc.Layers[a.LayerIndex].Name,groups=[doc.Groups.GroupName(i)for i in(a.GetGroupList()or[])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),
                selected=bool(obj.IsSelected(False)),grips_on=bool(obj.GripsOn),
                components=[dict(kind=str(c.ComponentIndexType),index=int(c.Index))for c in(obj.GetSelectedSubObjects()or[])]))
        return rows
    def archive():
        model=R.FileIO.File3dm()
        try:
            level=model.Layers.AddLayer('Default',S.Drawing.Color.Black)
            for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
                a=obj.Attributes.Duplicate()
                try:a.LayerIndex=level;a.RemoveFromAllGroups();model.Objects.Add(obj.Geometry,a)
                finally:a.Dispose()
            model.Settings.ModelUnitSystem=doc.ModelUnitSystem
            model.Settings.ModelAbsoluteTolerance=doc.ModelAbsoluteTolerance
            return S.Convert.ToBase64String(model.ToByteArray())
        finally:model.Dispose()
    old=doc.Layers.CurrentLayerIndex
    try:
        layer=doc.Layers.Add('Merge inputs',S.Drawing.Color.Blue);group=doc.Groups.Add('Merge input group')
        for i,definition in enumerate(case['sources']):
            mesh=G.Mesh();g=None
            try:
                mesh.Vertices.UseDoublePrecisionVertices=True
                for p in definition['vertices']:mesh.Vertices.Add(G.Point3d(*p))
                for f in definition['faces']:mesh.Faces.AddFace(*f)
                if definition['kind']=='subd':g=G.SubD.CreateFromMesh(mesh)
                else:
                    for j in range(mesh.Vertices.Count):
                        mesh.TextureCoordinates.Add(float(j),float(j*j));mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255,17*j%256,31*j%256,47*j%256))
                    mesh.Normals.ComputeNormals();g=mesh
                if g is None or not g.IsValid:raise ValueError('invalid owned source')
                a=R.DocObjects.ObjectAttributes()
                try:
                    a.Name='Input '+str(i);a.LayerIndex=layer;a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91)
                    if case.get('persistent_ids'):a.ObjectId=S.Guid.Parse(case['persistent_ids'][i])
                    a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                    key=doc.Objects.AddSubD(g,a)if definition['kind']=='subd'else doc.Objects.AddMesh(g,a)
                finally:a.Dispose()
                if key==S.Guid.Empty:raise ValueError('source insertion failed')
                keys.append(key)
            finally:
                if g is not None and g is not mesh:g.Dispose()
                mesh.Dispose()
        # Change the persistent option in an empty public getter before selecting
        # inputs; a preselection can otherwise finish before option tokens run.
        option=drive(host,case['results'])
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        record=doc.BeginUndoRecord('Merge earlier Undo witness')
        try:
            a=R.DocObjects.ObjectAttributes()
            try:a.Name='Undo witness';a.LayerIndex=layer;doc.Objects.AddPoint(G.Point3d(19,23,29),a)
            finally:a.Dispose()
        finally:doc.EndUndoRecord(record)
        selected=[]
        if case.get('witness_selected'):
            next(o for o in doc.Objects if o.Attributes.Name=='Undo witness').Select(True)
        for source_index in case.get('selection_order',list(range(len(keys)))):
            key,definition=keys[source_index],case['sources'][source_index]
            obj=doc.Objects.FindId(key);g=obj.Geometry
            for pick in definition['picks']:
                kind=pick['kind']
                if kind=='owner':obj.Select(True);continue
                if isinstance(g,G.SubD):
                    vertices=Scene.vertices(g);index={int(v.Id):j for j,v in enumerate(vertices)}
                    if kind=='edge':
                        pair=sorted(pick['vertices']);matches=[e for e in g.Edges if sorted([index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]])==pair]
                        if len(matches)!=1:raise ValueError('ambiguous SubD edge')
                        component=matches[0]
                    else:component=(list(g.Faces)if kind=='face'else vertices)[pick['index']]
                    ci=component.ComponentIndex()
                else:
                    if kind=='edge':
                        a,b=[g.TopologyVertices.TopologyVertexIndex(j)for j in pick['vertices']]
                        ci=G.ComponentIndex(G.ComponentIndexType.MeshTopologyEdge,g.TopologyEdges.GetEdgeIndex(a,b))
                    else:ci=G.ComponentIndex(G.ComponentIndexType.MeshFace if kind=='face'else G.ComponentIndexType.MeshVertex,pick['index'])
                if not case.get('postselect') and not obj.SelectSubObject(ci,True,True,False):raise ValueError('component selection failed')
                selected.append(dict(source=keys.index(key),kind=str(ci.ComponentIndexType),index=int(ci.Index)))
        before=snapshot();source_archive=archive();events=[]
        def ended(sender,event):
            if event.CommandEnglishName=='MergeFaces':events.append(str(event.CommandResult))
        marker='Owned MergeFaces command '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        R.Commands.Command.EndCommand+=ended
        try:
            if case.get('postselect'):
                from managed_component_input import drive as physical
                view=doc.Views.ActiveView;vp=view.ActiveViewport
                vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned MergeFaces',False)
                vp.DisplayMode=R.Display.DisplayModeDescription.FindByName('Shaded')
                bounds=G.BoundingBox([G.Point3d(*point)for source in case['sources']for point in source['vertices']])
                bounds.Inflate(.5);vp.ZoomBoundingBox(bounds);view.Redraw();R.RhinoApp.Wait()
                views=[]
                for reference in selected:
                    obj=doc.Objects.FindId(keys[reference['source']]);definition=case['sources'][reference['source']]
                    face=definition['faces'][reference['index']-1 if definition['kind']=='subd'else reference['index']]
                    point=G.Point3d(*[sum(definition['vertices'][j][k]for j in face)/float(len(face))for k in range(3)])
                    pixel=vp.WorldToClient(point);screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                    if definition['kind']=='subd':ci=next(f for f in obj.Geometry.Faces if int(f.Id)==reference['index']).ComponentIndex()
                    else:ci=G.ComponentIndex(G.ComponentIndexType.MeshFace,reference['index'])
                    views.append((screen,obj,[ci]))
                returned,interaction=physical(host,'_MergeFaces',op['id'],views,command='MergeFaces')
            else:interaction=drive(host)
        finally:R.Commands.Command.EndCommand-=ended
        if len(events)!=1:raise ValueError('MergeFaces did not finish exactly once')
        after=snapshot();output=archive();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        R.RhinoApp.RunScript('_Undo',False);undone=snapshot()
        R.RhinoApp.RunScript('_Redo',False);redone=snapshot()
        return dict(before=before,after=after,undone=undone,redone=redone,source_3dm=source_archive,after_3dm=output,
                    selected_input=selected,option_input=option,interaction=interaction,command_result=events[0],history=history),0
    finally:
        doc.Objects.UnselectAll();doc.Layers.SetCurrentLayerIndex(old,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        if layer>=0:doc.Layers.Delete(layer,True)
