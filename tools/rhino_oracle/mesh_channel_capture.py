"""Shared public mesh construction and channel snapshots for seam probes."""

def snapshot(mesh,host,case):
    value=host['_polygon_mesh_value'](mesh)
    value.update(uv=[[float(t.X),float(t.Y)] for t in mesh.TextureCoordinates],
                 normals=[[float(n.X),float(n.Y),float(n.Z)] for n in mesh.Normals],
                 colors=[[int(c.R),int(c.G),int(c.B)]+([255-int(c.A)] if 'transparency' in case else []) for c in mesh.VertexColors],
                 ngons=[dict(vertices=[int(i) for i in g.BoundaryVertexIndexList()],faces=[int(i) for i in g.FaceIndexList()]) for g in mesh.Ngons],
                 valid=bool(mesh.IsValid))
    value['topology_vertices']=[list(mesh.TopologyVertices.MeshVertexIndices(i)) for i in range(mesh.TopologyVertices.Count)]
    value['topology_edges']=[]
    for i in range(mesh.TopologyEdges.Count):
        p=mesh.TopologyEdges.GetTopologyVertices(i)
        value['topology_edges'].append(dict(vertices=[int(p.I),int(p.J)],faces=[int(f) for f in mesh.TopologyEdges.GetConnectedFaces(i)]))
    return value

def capture(case,host,change):
    R,S=host['Rhino'],host['System']
    mesh=R.Geometry.Mesh()
    try:
        mesh.Vertices.UseDoublePrecisionVertices=True
        for p in case['source']['vertices']: mesh.Vertices.Add(R.Geometry.Point3d(*p))
        for f in case['source']['faces']: mesh.Faces.AddFace(*f)
        if case['channels'] in ['all','uv']:
            for i in range(mesh.Vertices.Count): mesh.TextureCoordinates.Add(float(i),float(i*i))
        if case['channels'] in ['all','normals']:
            mesh.Normals.ComputeNormals()
            if case.get('custom_normals'):
                for i in range(mesh.Vertices.Count): mesh.Normals.SetNormal(i,R.Geometry.Vector3f(float(i+1),float(i%3-1),float(2-i%4)))
        if 'stored_normals' in case:
            for i,n in enumerate(case['stored_normals']):mesh.Normals.SetNormal(i,R.Geometry.Vector3f(*n))
        if case.get('unit_normals'):
            for i in range(mesh.Vertices.Count):
                n=R.Geometry.Vector3f(1.,0.,0.) if case['unit_normals']=='x' else R.Geometry.Vector3f(0.,0.,1. if i<3 else -1.)
                mesh.Normals.SetNormal(i,n)
        if case['channels']=='all':
            for i in range(mesh.Vertices.Count):
                alpha=255-case['transparency'][i] if 'transparency' in case else 255
                mesh.VertexColors.Add(S.Drawing.Color.FromArgb(alpha,(17*i)%256,(31*i)%256,(47*i)%256))
        if case.get('late_invalid_face'):
            n=mesh.Vertices.Count
            mesh.Faces.AddFace(n+2,n+3,n+4)
        before=snapshot(mesh,host,case)
        radial=[]
        if case.get('sort_radial'):
            for i in range(mesh.TopologyVertices.Count):
                original=list(mesh.TopologyVertices.ConnectedEdges(i) or [])
                success=bool(mesh.TopologyVertices.SortEdges(i))
                radial.append(dict(vertex=i,before=[int(e) for e in original],
                                   after=[int(e) for e in (mesh.TopologyVertices.ConnectedEdges(i) or [])],
                                   success=success))
        change(mesh)
        after=snapshot(mesh,host,case)
        result=dict(input=case,before=before,after=after)
        if radial:result['radial_sort']=radial
        return result,0
    finally: mesh.Dispose()
