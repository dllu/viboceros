"""Public empty-result validity, bounds, selection, and File3dm persistence queries."""

def capture(host, doc, ids, witness):
    R, S = host['Rhino'], host['System']
    objects = sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber)
    selected = [o.Id for o in objects if o.IsSelected(False)]
    empty, acceptance = [], []
    model = R.FileIO.File3dm()
    reloaded = None
    try:
        layer = model.Layers.AddLayer('Default', S.Drawing.Color.Black)
        for obj in objects:
            g = obj.Geometry
            if isinstance(g, R.Geometry.Mesh) and g.Vertices.Count == 0 and g.Faces.Count == 0:
                doc.Objects.UnselectAll()
                selectable = bool(doc.Objects.Select(obj.Id))
                empty.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                    valid=bool(g.IsValid), bounds_valid=bool(g.GetBoundingBox(True).IsValid),
                    selectable=selectable, selected=bool(obj.IsSelected(False))))
            a = obj.Attributes.Duplicate()
            try:
                a.LayerIndex = layer
                a.RemoveFromAllGroups()
                before = len(list(model.Objects))
                identifier = model.Objects.Add(g, a)
                acceptance.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                    witness=obj.Id == witness, added=identifier != S.Guid.Empty,
                    count_increased=len(list(model.Objects)) == before+1))
            finally:
                a.Dispose()
        encoded = model.ToByteArray()
        reloaded = R.FileIO.File3dm.FromByteArray(encoded)
        if reloaded is None:
            raise ValueError('empty-result archive could not be reloaded')
        return dict(empty=empty, acceptance=acceptance,
                    result_3dm=S.Convert.ToBase64String(encoded),
                    reloaded_meshes=[host['_polygon_mesh_value'](o.Geometry) for o in reloaded.Objects
                                     if isinstance(o.Geometry, R.Geometry.Mesh)])
    finally:
        if reloaded is not None:
            reloaded.Dispose()
        model.Dispose()
        doc.Objects.UnselectAll()
        for identifier in selected:
            doc.Objects.Select(identifier)
