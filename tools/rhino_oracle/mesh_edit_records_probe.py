"""Fixed public mesh grip edits and diagnostics in an empty owned document."""
import re

CASES = ('mixed_mirror', 'triangle_line', 'triangle_point', 'quad_edge', 'quad_line', 'quad_point')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op['op'] != 'mesh_edit_records' or not isinstance(op['id'], str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or op['case'] not in CASES):
        raise ValueError('invalid bounded mesh face-record recipe')


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='mesh_edit_records', id='mesh-records-'+case, case=case) for case in CASES])


def definition(case):
    if case == 'mixed_mirror':
        points = [[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.],[2.,0.,0.]]
        return points, [[0,1,2,3],[4,2,3]], {0:[-2.,0.,0.],2:[2.,0.,0.]}
    points = [[0.,0.,0.],[2.,0.,0.],[0.,2.,0.]]
    if case == 'triangle_line':
        return points, [[0,1,2]], {2:[1.,0.,0.]}
    if case == 'triangle_point':
        return points, [[0,1,2]], {1:points[0],2:points[0]}
    points = [[0.,0.,0.],[2.,0.,0.],[2.,2.,0.],[0.,2.,0.]]
    moves = {1:points[0]} if case == 'quad_edge' else {i:([float(i),0.,0.] if case == 'quad_line' else points[0]) for i in range(4)}
    return points, [[0,1,2,3]], moves


def run(op, host):
    validate(op)
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError('mesh records require an empty owned document')
    points, faces, moves = definition(op['case'])
    mesh = Rhino.Geometry.Mesh()
    mesh.Vertices.UseDoublePrecisionVertices = True
    for i, p in enumerate(points):
        mesh.Vertices.Add(host['_point'](p))
        mesh.VertexColors.Add(System.Drawing.Color.FromArgb(255, 11+i, 21+i, 31+i))
    for face in faces:
        mesh.Faces.AddFace(*face)

    def snapshot(obj):
        m = obj.Geometry
        m.FaceNormals.ComputeFaceNormals()
        mass = Rhino.Geometry.AreaMassProperties.Compute(m)
        try:
            metric = dict(area=float(mass.Area), centroid=host['_xyz'](mass.Centroid)) if mass is not None else None
        finally:
            if mass is not None:
                mass.Dispose()
        nearest = m.ClosestMeshPoint(host['_point']([1.,-1.,1.]), 0.)
        edges = []
        for i in range(m.TopologyEdges.Count):
            line = m.TopologyEdges.EdgeLine(i)
            edges.append(dict(points=[host['_xyz'](line.From),host['_xyz'](line.To)],
                              faces=list(m.TopologyEdges.GetConnectedFaces(i))))
        return dict(mesh=host['_polygon_mesh_value'](m), valid=bool(m.IsValid),
                    closed=bool(m.IsClosed), edges=edges,
                    face_normals=[host['_xyz'](m.FaceNormals[i]) for i in range(m.FaceNormals.Count)],
                    colors=[[int(c.R),int(c.G),int(c.B),255-int(c.A)] for c in m.VertexColors],
                    mass=metric, closest=dict(point=host['_xyz'](nearest.Point),face=int(nearest.FaceIndex)) if nearest is not None else None)

    try:
        key = doc.Objects.AddMesh(mesh)
        if key == System.Guid.Empty:
            raise ValueError('mesh record insertion failed')
        obj = doc.Objects.FindId(key)
        before = snapshot(obj)
        obj.GripsOn = True
        grips = obj.GetGrips()
        for i, p in sorted(moves.items()):
            grips[i].Move(host['_point'](p))
        obj = doc.Objects.GripUpdate(obj, True)
        if obj is None:
            raise ValueError('mesh record grip update failed')
        edited = snapshot(obj)
        obj.GripsOn = True
        grips = obj.GetGrips()
        for i, p in enumerate(points):
            grips[i].Move(host['_point'](p))
        obj = doc.Objects.GripUpdate(obj, True)
        if obj is None:
            raise ValueError('mesh record recovery failed')
        return dict(before=before, edited=edited, restored=snapshot(obj)), 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
        mesh.Dispose()
