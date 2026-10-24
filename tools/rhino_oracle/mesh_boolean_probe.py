"""Observe public Mesh.CreateBoolean* results without modifying input meshes."""
try:
    from .mesh_boolean_cases import ALL_CASES, validate
except (ImportError, ValueError):
    from mesh_boolean_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('owned empty idle document required')
    case = ALL_CASES[operation['case']]
    meshes, result = [], None
    try:
        for data in case['meshes']:
            mesh = G.Mesh()
            meshes.append(mesh)
            mesh.Vertices.UseDoublePrecisionVertices = True
            for p in data['vertices']:
                mesh.Vertices.Add(*p)
            for f in data['faces']:
                mesh.Faces.AddFace(*f)
            if not mesh.IsValid:
                raise ValueError('invalid mesh Boolean input')
        before = [int(mesh.DataCRC(0)) for mesh in meshes]
        source_meshes = [host['_polygon_mesh_value'](mesh) for mesh in meshes]
        options = G.MeshBooleanOptions()
        options.Tolerance = tolerance['absolute']
        if case['operation'] == 'union':
            result, status = G.Mesh.CreateBooleanUnion(S.Array[G.Mesh](meshes), options)
        else:
            method = (G.Mesh.CreateBooleanIntersection if case['operation'] == 'intersection'
                      else G.Mesh.CreateBooleanDifference)
            result, status = method(S.Array[G.Mesh](meshes[:1]), S.Array[G.Mesh](meshes[1:]), options)
        volumes, areas = [], []
        for mesh in result or []:
            props = G.VolumeMassProperties.Compute(mesh) if mesh.IsClosed else None
            try:
                volumes.append(float(props.Volume) if props is not None else None)
            finally:
                if props is not None:
                    props.Dispose()
            props = G.AreaMassProperties.Compute(mesh)
            try:
                areas.append(float(props.Area) if props is not None else None)
            finally:
                if props is not None:
                    props.Dispose()
        return dict(input=case, returned_null=result is None, status=str(status),
                    source_unchanged=before == [int(mesh.DataCRC(0)) for mesh in meshes],
                    source_meshes_before=source_meshes,
                    source_meshes_after=[host['_polygon_mesh_value'](mesh) for mesh in meshes],
                    meshes=[host['_polygon_mesh_value'](mesh) for mesh in result or []],
                    volumes=volumes, areas=areas), 0
    finally:
        for mesh in meshes + list(result or []):
            mesh.Dispose()
