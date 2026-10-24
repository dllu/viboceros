"""Public Mesh.Split controls, using only private duplicate mesh instances."""


def mesh_value(mesh,host,channels=False):
    value=host['_polygon_mesh_value'](mesh)
    value['ngons']=[dict(vertices=[int(i)for i in g.BoundaryVertexIndexList()],
                         faces=[int(i)for i in g.FaceIndexList()])for g in mesh.Ngons]
    if channels:
        value['uv']=[[float(p.X),float(p.Y)]for p in mesh.TextureCoordinates]or None
        value['vertex_normals']=[[float(p.X),float(p.Y),float(p.Z)]for p in mesh.Normals]or None
        value['colors']=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in mesh.VertexColors]or None
    return value


def capture(geometry,case,host):
    G,S=host['Rhino'].Geometry,host['System']
    rows=[]
    for index in case['first']:
        target=geometry[index].DuplicateMesh()
        cutters=[geometry[i].DuplicateMesh()for i in case['second']]
        parts=[]
        try:
            before=[int(m.DataCRC(0))for m in [target]+cutters]
            parts=target.Split(S.Array[G.Mesh](cutters),float(case.get('sdk_tolerance',1e-10)),
                bool(case.get('split_coplanar',True)),bool(case.get('create_ngons',True)),
                None,S.Threading.CancellationToken(),None)or []
            rows.append(dict(parts=[mesh_value(m,host)for m in parts],
                inputs_unchanged=before==[int(m.DataCRC(0))for m in [target]+cutters]))
        finally:
            for m in list(parts)+[target]+cutters:m.Dispose()
    return rows
