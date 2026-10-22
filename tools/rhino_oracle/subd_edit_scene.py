"""Owned SubD editing scenes, snapshots and public 3DM archive export."""
from copy import deepcopy


class Scene:
    def __init__(self, case, host):
        self.case = deepcopy(case)
        self.host = host
        self.R, self.S = host['Rhino'], host['System']
        self.doc = self.R.RhinoDoc.ActiveDoc
        self.ids, self.layers, self.group = [], [], -1
        self.old_layer = self.doc.Layers.CurrentLayerIndex

    @staticmethod
    def vertices(subd):
        result, v = [], subd.Vertices.First
        while v is not None:
            result.append(v)
            v = v.Next
        return result

    def geometry(self, definition):
        R = self.R
        if 'native_archive' in definition:
            seed = definition['native_archive']
            model = R.FileIO.File3dm.FromByteArray(self.S.Convert.FromBase64String(seed['data']))
            if model is None:
                raise ValueError('cannot read native SubD seed')
            try:
                matches = [o.Geometry for o in model.Objects if isinstance(o.Geometry, R.Geometry.SubD)
                           and o.Attributes.Name == seed['object_name']]
                if len(matches) != 1:
                    raise ValueError('native SubD seed is ambiguous')
                return matches[0].Duplicate()
            finally:
                model.Dispose()
        mesh = R.Geometry.Mesh()
        mesh.Vertices.UseDoublePrecisionVertices = True
        try:
            for point in definition['vertices']:
                mesh.Vertices.Add(R.Geometry.Point3d(*point))
            for face in definition['faces']:
                mesh.Faces.AddFace(*face)
            return R.Geometry.SubD.CreateFromMesh(mesh)
        finally:
            mesh.Dispose()

    def __enter__(self):
        R, S, doc = self.R, self.S, self.doc
        if list(doc.Objects) or doc.UndoRecordingIsActive:
            raise ValueError('empty owned idle document required')
        try:
            serial = doc.BeginUndoRecord('Owned SubD edit setup')
            try:
                for name, color in [('SubD edit input', S.Drawing.Color.Blue), ('SubD edit current', S.Drawing.Color.Green)]:
                    self.layers.append(doc.Layers.Add(name, color))
                self.group = doc.Groups.Add('SubD edit assembly')
                for index, definition in enumerate(self.case['sources']):
                    subd = self.geometry(definition)
                    if subd is None:
                        raise ValueError('cannot construct source SubD')
                    try:
                        vertices = self.vertices(subd)
                        indices = {int(v.Id): i for i, v in enumerate(vertices)}
                        for i in definition.get('corners', []):
                            vertices[i].Tag = R.Geometry.SubDVertexTag.Corner
                        for edge in subd.Edges:
                            pair = sorted([indices[int(edge.VertexFrom.Id)], indices[int(edge.VertexTo.Id)]])
                            if pair in definition.get('creases', []):
                                edge.Tag = R.Geometry.SubDEdgeTag.Crease
                        subd.UpdateAllTagsAndSectorCoefficients()
                        attrs = R.DocObjects.ObjectAttributes()
                        attrs.LayerIndex, attrs.Name = self.layers[0], 'Input ' + str(index)
                        attrs.ObjectColor = S.Drawing.Color.FromArgb(17, 43, 91)
                        attrs.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                        attrs.AddToGroup(self.group)
                        self.ids.append(doc.Objects.AddSubD(subd, attrs))
                    finally:
                        subd.Dispose()
                doc.Layers.SetCurrentLayerIndex(self.layers[1], True)
            finally:
                doc.EndUndoRecord(serial)
            for key, definition in zip(self.ids, self.case['sources']):
                if 'soft' not in definition:
                    continue
                obj = doc.Objects.FindId(key)
                vertices = self.vertices(obj.Geometry)
                indices = {int(v.Id): i for i, v in enumerate(vertices)}
                for edge in obj.Geometry.Edges:
                    pair = sorted([indices[int(edge.VertexFrom.Id)], indices[int(edge.VertexTo.Id)]])
                    if pair in definition.get('seed_edges', definition['edges']):
                        obj.SelectSubObject(edge.ComponentIndex(), True, True, False)
                completed = []
                def ended(sender, event):
                    if event.CommandEnglishName == 'SubDCrease':
                        completed.append(str(event.CommandResult))
                R.Commands.Command.EndCommand += ended
                try:
                    from number_token import number_token
                    R.RhinoApp.RunScript('_-SubDCrease ' + number_token(definition['soft']) + ' _Enter _Cancel _Cancel', True)
                finally:
                    R.Commands.Command.EndCommand -= ended
                if completed != ['Success']:
                    raise ValueError('source soft crease seed failed: ' + str(completed))
                for obj in list(doc.Objects):
                    obj.UnselectAllSubObjects()
                    obj.Select(False)
            doc.ClearUndoRecords(True)
            serial = doc.BeginUndoRecord('Owned SubD edit Undo witness')
            try:
                attrs = R.DocObjects.ObjectAttributes()
                attrs.Name, attrs.LayerIndex = 'Undo witness', self.layers[1]
                doc.Objects.AddPoint(R.Geometry.Point3d(19., 23., 29.), attrs)
            finally:
                doc.EndUndoRecord(serial)
            return self
        except Exception:
            self.close()
            raise

    def select(self):
        for key, definition in zip(self.ids, self.case['sources']):
            obj = self.doc.Objects.FindId(key)
            if self.case.get('grips'):
                obj.GripsOn = True
                grips = list(obj.GetGrips() or [])
                for index in definition.get('selected_grips', []):
                    grips[index].Select(True)
            if self.case.get('whole'):
                obj.Select(True)
                continue
            vertices = self.vertices(obj.Geometry)
            if self.case.get('vertex_only'):
                obj.SelectSubObject(vertices[0].ComponentIndex(), True, True, False)
                continue
            indices = {int(v.Id): i for i, v in enumerate(vertices)}
            for edge in obj.Geometry.Edges:
                pair = sorted([indices[int(edge.VertexFrom.Id)], indices[int(edge.VertexTo.Id)]])
                if pair in definition['edges'] and not obj.SelectSubObject(edge.ComponentIndex(), True, True, False):
                    raise ValueError('source edge preselection failed')

    def snapshot(self):
        from subd_box_probe import subd_value
        rows = []
        for obj in sorted(list(self.doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            attrs, g = obj.Attributes, obj.Geometry
            row = dict(source=self.ids.index(obj.Id) if obj.Id in self.ids else None,
                       name=attrs.Name, layer=self.doc.Layers[attrs.LayerIndex].Name,
                       groups=[self.doc.Groups.GroupName(i) for i in (attrs.GetGroupList() or [])],
                       color=[int(attrs.ObjectColor.R), int(attrs.ObjectColor.G), int(attrs.ObjectColor.B)],
                       color_source=int(attrs.ColorSource), selected=bool(obj.IsSelected(False)),
                       grips_on=bool(obj.GripsOn),
                       selected_grips=[int(grip.Index) for grip in (obj.GetGrips() or []) if grip.IsSelected(False)] if obj.GripsOn else [],
                       crc=int(g.DataCRC(0)))
            if isinstance(g, self.R.Geometry.SubD):
                row['subd'] = subd_value(g, self.host)
                row['selected_edges'] = [i for i, e in enumerate(g.Edges) if obj.IsSubObjectSelected(e.ComponentIndex())]
                row['selected_vertices'] = [i for i, v in enumerate(self.vertices(g)) if obj.IsSubObjectSelected(v.ComponentIndex())]
            else:
                row['point'] = self.host['_xyz'](g.Location)
            rows.append(row)
        return rows

    def archive(self):
        model = self.R.FileIO.File3dm()
        try:
            layer = model.Layers.AddLayer('Default', self.S.Drawing.Color.Black)
            for obj in sorted(list(self.doc.Objects), key=lambda o: o.RuntimeSerialNumber):
                attrs = obj.Attributes.Duplicate()
                attrs.LayerIndex = layer
                attrs.RemoveFromAllGroups()
                model.Objects.Add(obj.Geometry, attrs)
            model.Settings.ModelAbsoluteTolerance = self.doc.ModelAbsoluteTolerance
            model.Settings.ModelUnitSystem = self.doc.ModelUnitSystem
            return self.S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

    def close(self):
        self.doc.Layers.SetCurrentLayerIndex(self.old_layer, True)
        for obj in list(self.doc.Objects):
            self.doc.Objects.Delete(obj.Id, True)
        if self.group >= 0:
            self.doc.Groups.Delete(self.group)
        for layer in reversed(self.layers):
            self.doc.Layers.Delete(layer, True)

    def __exit__(self, kind, error, traceback):
        self.close()
