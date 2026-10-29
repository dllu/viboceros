"""Observe actual CollapseMeshFace in an empty, owned idle document."""
import math
import re


def validate(op):
    fields = {'op', 'id', 'vertices', 'faces', 'face', 'channels'}
    if op.get('op') in ['mesh_face_collapse_precision','mesh_face_collapse_buffers']:
        fields.add('precision')
    if op.get('op') == 'mesh_face_collapse_mode':
        fields.add('display_mode')
    if set(op) != fields or op['op'] not in ['mesh_face_collapse', 'mesh_face_collapse_point', 'mesh_face_collapse_click', 'mesh_face_collapse_precision', 'mesh_face_collapse_getter', 'mesh_face_collapse_shaded', 'mesh_face_collapse_mode', 'mesh_face_collapse_buffers']:
        raise ValueError('bounded face collapse recipe required')
    if 'display_mode' in op and op['display_mode'] not in ['Wireframe','Shaded']:
        raise ValueError('known owned display mode required')
    if 'precision' in op and op['precision'] not in ['single','double']:
        raise ValueError('known mesh precision required')
    if not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None:
        raise ValueError('bounded operation id required')
    vertices, faces = op['vertices'], op['faces']
    if not 3 <= len(vertices) <= 128 or not 1 <= len(faces) <= 128:
        raise ValueError('bounded mesh required')
    if any(len(p) != 3 or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in p) for p in vertices):
        raise ValueError('finite vertices required')
    if any(len(f) not in [3, 4] or any(type(i) is not int or not 0 <= i < len(vertices) for i in f) for f in faces):
        raise ValueError('valid face indices required')
    if type(op['face']) is not int or not 0 <= op['face'] < len(faces) or op['channels'] not in ['none', 'all']:
        raise ValueError('valid face and channels required')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    mesh = G.Mesh()
    key = S.Guid.Empty
    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = R.DocObjects.ViewportInfo(vp)
    original_mode = vp.DisplayMode
    def value(geometry):
        if geometry is None:
            return None
        result = host['_polygon_mesh_value'](geometry)
        result.update(colors=[[int(c.R), int(c.G), int(c.B), int(c.A)] for c in geometry.VertexColors],
                      uv=[[float(p.X), float(p.Y)] for p in geometry.TextureCoordinates],
                      normals=[[float(n.X), float(n.Y), float(n.Z)] for n in geometry.Normals],
                      ngons=[dict(vertices=list(n.BoundaryVertexIndexList()), faces=list(n.FaceIndexList())) for n in geometry.Ngons])
        if op['op'] in ['mesh_face_collapse_precision','mesh_face_collapse_mode']:
            result.update(float_vertices=[[float(p.X),float(p.Y),float(p.Z)] for p in geometry.Vertices],
                          double_precision=bool(geometry.Vertices.UseDoublePrecisionVertices),
                          valid=bool(geometry.IsValid))
        if op['op'] == 'mesh_face_collapse_buffers':
            crc = int(geometry.DataCRC(0))
            was_double = bool(geometry.Vertices.UseDoublePrecisionVertices)
            copy = geometry.DuplicateMesh()
            try:
                copy.Vertices.UseDoublePrecisionVertices = False
                if copy.Vertices.UseDoublePrecisionVertices:
                    raise ValueError('float-only snapshot still has double vertices')
                result['raw_float_vertices'] = [[float(p.X),float(p.Y),float(p.Z)] for p in copy.Vertices]
            finally:
                copy.Dispose()
            result['double_precision'] = was_double
            result['source_unchanged_by_read'] = (int(geometry.DataCRC(0)) == crc and
                bool(geometry.Vertices.UseDoublePrecisionVertices) == was_double)
            if not result['source_unchanged_by_read']:
                raise ValueError('position buffer snapshot mutated its source')
            result['valid'] = bool(geometry.IsValid)
        return result
    try:
        mesh.Vertices.UseDoublePrecisionVertices = op.get('precision','double') == 'double'
        for p in op['vertices']:
            mesh.Vertices.Add(G.Point3d(*p))
        for f in op['faces']:
            mesh.Faces.AddFace(*f)
        if op['channels'] == 'all':
            for i in range(mesh.Vertices.Count):
                mesh.TextureCoordinates.Add(float(i), float(i*i))
                mesh.VertexColors.Add(S.Drawing.Color.FromArgb(255, 17*i % 256, 31*i % 256, 47*i % 256))
            mesh.Normals.ComputeNormals()
        if op['op'] == 'mesh_face_collapse_buffers':
            mesh.Vertices.UseDoublePrecisionVertices = op['precision'] == 'double'
        authored_double = bool(mesh.Vertices.UseDoublePrecisionVertices)
        if not mesh.IsValid:
            raise ValueError('invalid authored mesh')
        attr = R.DocObjects.ObjectAttributes()
        try:
            attr.Name = 'Collapse source'
            key = doc.Objects.AddMesh(mesh, attr)
        finally:
            attr.Dispose()
        if key == S.Guid.Empty:
            raise ValueError('owned mesh insertion failed')
        # A no-op must never undo an earlier case in this owned batch.
        doc.ClearUndoRecords(True)
        doc.ClearRedoRecords()
        obj = doc.Objects.FindId(key)
        before = value(obj.Geometry)
        component = G.ComponentIndex(G.ComponentIndexType.MeshFace, op['face'])
        if op['op'] in ['mesh_face_collapse','mesh_face_collapse_precision','mesh_face_collapse_buffers'] and not obj.SelectSubObject(component, True, True, False):
            raise ValueError('owned face selection failed')
        events = []
        def ended(sender, event):
            if event.CommandEnglishName == 'CollapseMeshFace':
                events.append(str(event.CommandResult))
        R.Commands.Command.EndCommand += ended
        marker = 'Viboceros face collapse '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        input_trace = None
        try:
            if op['op'] in ['mesh_face_collapse_getter','mesh_face_collapse_shaded','mesh_face_collapse_mode']:
                if op['op'] in ['mesh_face_collapse_shaded','mesh_face_collapse_mode']:
                    mode = R.Display.DisplayModeDescription.FindByName(op.get('display_mode','Shaded'))
                    if mode is None: raise ValueError('owned shaded mode missing')
                    vp.DisplayMode = mode
                vp.SetProjection(R.Display.DefinedViewportProjection.Top, 'Owned mesh face input', False)
                bounds = mesh.GetBoundingBox(True)
                bounds.Inflate(1.)
                vp.ZoomBoundingBox(bounds)
                view.Redraw()
                point = [sum(op['vertices'][i][axis] for i in op['faces'][op['face']])/float(len(op['faces'][op['face']])) for axis in range(3)]
                from conic_getter_input import drive
                success, input_trace = drive(dict(id=op['id'],actions=[dict(click=point), 'Enter']),host,[],command='CollapseMeshFace')
                if not success or input_trace['errors']:
                    raise ValueError('face input incomplete: '+str(input_trace))
                macro = None
            elif op['op'] == 'mesh_face_collapse_click':
                if not vp.SetProjection(R.Display.DefinedViewportProjection.Top, 'Owned face collapse', False):
                    raise ValueError('owned face view failed')
                bounds = mesh.GetBoundingBox(True)
                bounds.Inflate(1.)
                vp.ZoomBoundingBox(bounds)
                point = [sum(op['vertices'][i][axis] for i in op['faces'][op['face']])/float(len(op['faces'][op['face']])) for axis in range(3)]
                pixel = vp.WorldToClient(G.Point3d(*point))
                screen = view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
                doc.Views.Redraw()
                host['_record_progress']('PICK @face-collapse:%s %d %d' % (op['id'],screen.X,screen.Y))
                macro = '_CollapseMeshFace _Pause _Enter'
            elif op['op'] == 'mesh_face_collapse_point':
                # A documented point token follows the face getter.
                point = [sum(op['vertices'][i][axis] for i in op['faces'][op['face']])/float(len(op['faces'][op['face']])) for axis in range(3)]
                token = 'w'+','.join(format(x, '.17g') for x in point)
                macro = '_CollapseMeshFace '+token+' _Enter'
            else:
                macro = '_CollapseMeshFace _Enter'
            if macro is not None:
                R.RhinoApp.RunScript(macro, True)
        finally:
            R.Commands.Command.EndCommand -= ended
        if len(events) != 1 or events[0] not in ['Success', 'Nothing']:
            raise ValueError('collapse did not finish: '+str(events))
        objects = list(doc.Objects)
        if len(objects) > 1:
            raise ValueError('unexpected collapse outputs')
        after = value(objects[0].Geometry) if objects else None
        attributes_preserved = not objects or objects[0].Attributes.Name == 'Collapse source'
        R.RhinoApp.RunScript('_Undo', False)
        restored = list(doc.Objects)
        undo = len(restored) == 1 and value(restored[0].Geometry) == before
        R.RhinoApp.RunScript('_Redo', False)
        redone = list(doc.Objects)
        redo = (value(redone[0].Geometry) if redone else None) == after
        return dict(before=before, after=after, attributes_preserved=attributes_preserved,
                    undo=undo, redo=redo, command_result=events[0],
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1],
                    input_trace=input_trace, display_mode_id=str(vp.DisplayMode.Id),
                    authored_double_precision=authored_double,
                    native_invalid_mesh_warning='Output mesh is invalid' in R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]), 0
    finally:
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        vp.DisplayMode = original_mode
        vp.SetViewProjection(original, False)
        original.Dispose()
        mesh.Dispose()
