"""Owned Rhino mesh Boolean command scenes and EndCommand snapshots."""
if __package__:
    from .mesh_boolean_command_cases import ALL_CASES, validate
else:
    from mesh_boolean_command_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance, case_table=None, command_name=None):
    if case_table is None:
        validate(operation)
        case_table = ALL_CASES
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    case = case_table[operation['case']]
    name = command_name or 'MeshBoolean'+case['operation'].capitalize()
    ids, layers, groups, geometry = [], [], [], []
    witness, old_layer = None, doc.Layers.CurrentLayerIndex
    old_tolerance = doc.ModelAbsoluteTolerance

    def xyz(p):
        return [float(p.X), float(p.Y), float(p.Z)]

    def make(data):
        if data['kind'] == 'point':
            return G.Point(G.Point3d(*data['point']))
        if data['kind'] == 'polyline':
            return G.PolylineCurve([G.Point3d(*p) for p in data['points']])
        if data['kind'] == 'brep_box':
            points = [G.Point3d(*p) for p in data['vertices']]
            return G.Brep.CreateFromBox(G.BoundingBox(points))
        mesh = G.Mesh()
        mesh.Vertices.UseDoublePrecisionVertices = True
        for point in data['vertices']:
            mesh.Vertices.Add(*point)
        for face in data['faces']:
            mesh.Faces.AddFace(*face)
        if case.get('normals'):
            mesh.Normals.ComputeNormals()
        return mesh

    def snapshot():
        result = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            a, g = obj.Attributes, obj.Geometry
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                       role='source' if obj.Id in ids else 'witness' if obj.Id == witness else 'output',
                       selected=bool(obj.IsSelected(False)), name=a.Name, layer=doc.Layers[a.LayerIndex].Name,
                       groups=sorted(doc.Groups.GroupName(i) for i in a.GetGroupList() or []),
                       color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                       color_source=int(a.ColorSource), crc=int(g.DataCRC(0)))
            if isinstance(g, G.Mesh):
                if case.get('capture_mesh_split_sdk'):
                    from mesh_split_sdk import mesh_value
                    mesh_data = mesh_value(g,host)
                else:
                    mesh_data = host['_polygon_mesh_value'](g)
                row.update(mesh=mesh_data, vertex_normals=int(g.Normals.Count),
                           face_normals=int(g.FaceNormals.Count), closed=bool(g.IsClosed))
            elif isinstance(g, G.Point):
                row['point'] = xyz(g.Location)
            elif isinstance(g, G.Curve):
                ok, polyline = g.TryGetPolyline()
                if ok:
                    row['polyline'] = [xyz(p) for p in polyline]
                else:
                    row['kind'] = str(g.GetType().Name)
            else:
                row['kind'] = str(g.GetType().Name)
            result.append(row)
        return result

    try:
        if 'document_tolerance' in case:
            doc.ModelAbsoluteTolerance = float(case['document_tolerance'])
        record = doc.BeginUndoRecord('Owned mesh Boolean sources')
        try:
            for index in range(len(case['sources'])+1):
                layer = R.DocObjects.Layer()
                layer.Name = 'Boolean source '+str(index) if index < len(case['sources']) else 'Boolean output'
                layer.Color = S.Drawing.Color.FromArgb(91, 109, 121)
                layers.append(doc.Layers.Add(layer))
                layer.Dispose()
            for index, data in enumerate(case['sources']):
                g = make(data)
                geometry.append(g)
                if not g.IsValid:
                    raise ValueError('invalid mesh Boolean source')
                a = R.DocObjects.ObjectAttributes()
                try:
                    a.LayerIndex, a.Name = layers[index], 'Source '+str(index)
                    a.ObjectColor = S.Drawing.Color.FromArgb(13+index, 27, 53)
                    a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                    identifier = doc.Objects.Add(g, a)
                    if identifier == S.Guid.Empty:
                        raise ValueError('mesh Boolean source admission failed')
                    ids.append(identifier)
                finally:
                    a.Dispose()
            for index, members in enumerate([[identifier] for identifier in ids]+[ids]):
                groups.append(doc.Groups.Add('Boolean group '+str(index), members))
            doc.Layers.SetCurrentLayerIndex(layers[-1], True)
        finally:
            doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True)
        record = doc.BeginUndoRecord('Owned mesh Boolean Undo witness')
        try:
            a = R.DocObjects.ObjectAttributes()
            a.Name, a.LayerIndex = 'Undo witness', layers[-1]
            witness = doc.Objects.AddPoint(G.Point3d(99., 98., 97.), a)
            a.Dispose()
        finally:
            doc.EndUndoRecord(record)
        first = case.get('first', case.get('selected',
            [0] if case['operation'] == 'difference' else list(range(len(ids)))))
        if case['preselect']:
            for i in first:
                doc.Objects.Select(ids[i])
        model = R.FileIO.File3dm()
        try:
            layer = model.Layers.AddLayer('Default', S.Drawing.Color.Black)
            for identifier in ids:
                obj = doc.Objects.FindId(identifier)
                a = obj.Attributes.Duplicate()
                a.LayerIndex = layer
                a.RemoveFromAllGroups()
                model.Objects.Add(obj.Geometry, a)
                a.Dispose()
            model.Settings.ModelAbsoluteTolerance = doc.ModelAbsoluteTolerance
            source_3dm = S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()
        before = snapshot()
        intersection_sdk = None
        if case.get('capture_intersection_sdk'):
            from mesh_intersect_sdk import capture
            intersection_sdk = capture(geometry,host,doc.ModelAbsoluteTolerance*float(
                G.Intersect.Intersection.MeshIntersectionsTolerancesCoefficient))
        split_sdk = None
        if case.get('capture_mesh_split_sdk'):
            from mesh_split_sdk import capture
            split_sdk = capture(geometry,case,host)
        macro = '_-'+name+' '+case.get('arguments', '')
        if case.get('cancel'):
            macro += ' _Cancel'
        else:
            if not case['preselect']:
                macro += ' '+' '.join('_SelID '+str(ids[i]) for i in first)+' _Enter'
            if case['operation'] in ('difference', 'split') or (case['operation'] == 'intersection' and 'first' in case):
                macro += ' '+case.get('second_arguments', '')
                if case.get('cancel_second'):
                    macro += ' _Cancel'
                else:
                    second = case.get('second', list(range(1, len(ids))))
                    macro += ' '+' '.join('_SelID '+str(ids[i]) for i in second)+' _Enter'
            macro += ' _Enter _Cancel'
        marker = 'Owned mesh Boolean '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(R.Commands.Command, name,
            lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        after_script = snapshot()
        empty_storage = None
        if case.get('capture_empty_storage'):
            from mesh_empty_capture import capture
            empty_storage = capture(host, doc, ids, witness)
        R.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        result = dict(input=case, before=before, after=after, after_script=after_script,
                    undo=undo, redo=redo, success=success, events=events, history=history,
                    empty_storage=empty_storage, source_3dm=source_3dm, document_tolerance=float(doc.ModelAbsoluteTolerance),
                    tolerance_policy=dict(zero=float(R.RhinoMath.ZeroTolerance),
                        coefficient=float(G.Intersect.Intersection.MeshIntersectionsTolerancesCoefficient)))
        if intersection_sdk is not None:result['intersection_sdk']=intersection_sdk
        if split_sdk is not None:result['split_sdk']=split_sdk
        return result, 0
    finally:
        doc.ModelAbsoluteTolerance = old_tolerance
        doc.Layers.SetCurrentLayerIndex(old_layer, True)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        doc.ClearUndoRecords(True)
        for index in reversed(groups):
            doc.Groups.Delete(index)
        for index in reversed(layers):
            doc.Layers.Delete(index, True)
        for g in geometry:
            g.Dispose()
