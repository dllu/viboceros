"""Public Inset command on owned face selections; no proprietary implementation access."""
if __package__:
    from .inset_cases import TARGETS,EXTRA_TARGETS,scenes,validate
    from .subd_join_probe import source as mesh_source
else:
    from inset_cases import TARGETS,EXTRA_TARGETS,scenes,validate
    from subd_join_probe import source as mesh_source


def source(net,host):
    if not net.get('direct'):return mesh_source(net,host)
    R=host['Rhino'];G=R.Geometry
    points=[G.Point3d(*p)for p in net['points']];points.append(points[0])
    mesh=G.Mesh.CreateFromClosedPolyline(G.Polyline(points))
    if mesh is None:raise ValueError('cannot triangulate owned polygon')
    try:
        mesh.Ngons.AddPlanarNgons(1e-8);subd=G.SubD.CreateFromMesh(mesh)
    finally:mesh.Dispose()
    if subd is None:raise ValueError('cannot create owned polygon SubD')
    if subd.Faces.Count!=1:subd.MergeAllCoplanarFaces(1e-8)
    if not subd.IsValid or subd.Faces.Count!=1:
        subd.Dispose();raise ValueError('owned polygon did not create one SubD face')
    return subd


def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(op,host,iterations,tolerance,'subd')


def mesh_source_geometry(net,host,channels):
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if net.get('direct'):
        points=[G.Point3d(*p)for p in net['points']];points.append(points[0]);mesh=G.Mesh.CreateFromClosedPolyline(G.Polyline(points));mesh.Ngons.AddPlanarNgons(1e-8)
    else:
        mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
        for p in net['points']:mesh.Vertices.Add(G.Point3d(*p))
        for f in net['faces']:mesh.Faces.AddFace(*f)
    mesh.Normals.ComputeNormals()
    if channels:
        for i in range(mesh.Vertices.Count):mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255,20+30*i,40+20*i,60+10*i))
        for u,v in ((0.,0.),(1.,0.),(1.,1.),(0.,1.)):mesh.TextureCoordinates.Add(u,v)
    return mesh


def run_scene(op,host,iterations,tolerance,kind):
    from subd_box_probe import subd_value
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];doc=R.RhinoDoc.ActiveDoc;key=None;layer=-1;group=-1
    if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    targets=dict(TARGETS);targets.update(EXTRA_TARGETS);scene,indices=targets['quad'if op['target']=='quad_channels'else op['target']]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes;g=obj.Geometry
            row=dict(source=0 if obj.Id==key else None,selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),groups=[doc.Groups.GroupName(i)for i in(a.GetGroupList()or[])])
            if kind=='subd':
                row['selected_faces']=[i for i,f in enumerate(g.Faces)if obj.IsSubObjectSelected(f.ComponentIndex())];row['subd']=subd_value(g,host)
            else:
                row['selected_faces']=[i for i in range(g.Faces.Count)if obj.IsSubObjectSelected(R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i))]
                row['mesh']=host['_polygon_mesh_value'](g)
                row['mesh']['has_colors']=bool(g.VertexColors.Count==g.Vertices.Count);row['mesh']['valid']=bool(g.IsValid)
                row['mesh']['normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in g.Normals]
                row['mesh']['uv']=[[float(t.X),float(t.Y)]for t in g.TextureCoordinates]
                row['mesh']['colors']=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in g.VertexColors]
                row['mesh']['ngons']=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in g.Ngons]
            rows.append(row)
        return rows
    try:
        record=doc.BeginUndoRecord('Owned inset setup')
        try:
            layer=doc.Layers.Add('Inset source',S.Drawing.Color.Blue);group=doc.Groups.Add('Inset group')
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layer;a.Name='Input';a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
            subd=source(scenes()[scene],host)if kind=='subd'else mesh_source_geometry(scenes()[scene],host,op['target']=='quad_channels');model=R.FileIO.File3dm()
            try:
                if kind=='subd':model.Objects.AddSubD(subd);key=doc.Objects.AddSubD(subd,a)
                else:model.Objects.AddMesh(subd);key=doc.Objects.AddMesh(subd,a)
                archive=[int(b)for b in model.ToByteArray()]
            finally:model.Dispose();subd.Dispose();a.Dispose()
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);obj=doc.Objects.FindId(key);faces=list(obj.Geometry.Faces)
        if indices is None:doc.Objects.Select(key)
        for i in (indices or []):
            component=faces[i].ComponentIndex()if kind=='subd'else R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i)
            if not obj.SelectSubObject(component,True,True,False):raise ValueError('inset face preselection failed')
        if op['op']=='inset_mesh_ready':
            crc=int(obj.Geometry.DataCRC(0));obj.UnselectAllSubObjects();doc.Objects.UnselectAll()
            duplicate=obj.Geometry.DuplicateMesh()
            try:temporary=doc.Objects.AddMesh(duplicate)
            finally:duplicate.Dispose()
            for i in indices:doc.Objects.FindId(temporary).SelectSubObject(R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i),True,True,False)
            R.RhinoApp.RunScript('_Inset '+str(op['distance'])+' _Enter _Cancel _Cancel',True)
            if R.Commands.Command.InCommand()or int(doc.Objects.FindId(key).Geometry.DataCRC(0))!=crc:raise ValueError('mesh inset preference setup edited test source')
            doc.Objects.Delete(temporary,True);doc.ClearUndoRecords(True);obj=doc.Objects.FindId(key)
            for i in indices:obj.SelectSubObject(R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i),True,True,False)
        if op['op']=='inset_mesh_primed':
            crc=int(obj.Geometry.DataCRC(0))
            # A changed mesh distance reprompts but retains its old scripted
            # preview. Prime preferences through canceled owned commands, then
            # measure the actual command with the requested value as its default.
            for value in (op['distance']+.123456789,op['distance']):
                obj=doc.Objects.FindId(key)
                for i in indices:obj.SelectSubObject(R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i),True,True,False)
                R.RhinoApp.RunScript('_Inset '+str(value)+' _Cancel _Cancel',True)
                if R.Commands.Command.InCommand()or int(doc.Objects.FindId(key).Geometry.DataCRC(0))!=crc:raise ValueError('mesh inset preference priming edited owned source')
            doc.ClearUndoRecords(True);obj=doc.Objects.FindId(key)
            for i in indices:obj.SelectSubObject(R.Geometry.ComponentIndex(R.Geometry.ComponentIndexType.MeshFace,i),True,True,False)
        before=snapshot();marker='Owned inset '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_Inset '+('_Mode=_'+op['mode']+' 'if kind=='subd'and op['mode']is not None and (len(faces)if indices is None else len(indices))>1 else '')
        macro+=('_Cancel'if op.get('cancel')else str(op['distance']))+(' '+str(op['distance'])if op['op']=='inset_mesh_confirmed'else '')+(' _Cancel'if op.get('cancel')else ' _Enter _Cancel _Cancel')
        success,after,events=observe_command(R.Commands.Command,'Inset',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script=snapshot()
        if R.Commands.Command.InCommand():raise ValueError('Inset left an active command')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,archive=archive,before=before,after=after,after_script=after_script,undo=undo,redo=redo,history=history,results=[e['result']for e in events if e['name']=='Inset']),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        if layer>=0:doc.Layers.Delete(layer,True)
