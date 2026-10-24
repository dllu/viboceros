"""Owned analytic sources shared by point extraction probes."""
def create(kind, host):
    from grip_navigation_probe import create_source
    G = host["Rhino"].Geometry
    if kind == 'line':
        return G.LineCurve(G.Point3d(0.,0.,0.), G.Point3d(8.,4.,2.))
    if kind == 'brep_periodic_joined':
        first = create('brep_periodic_u', host)
        second = first.DuplicateBrep()
        try:
            second.Transform(G.Transform.Translation(0.,0.,2.))
            joined = G.Brep.JoinBreps([first, second], 1.e-6)
            if joined is None or len(joined)!=1:
                raise ValueError('owned periodic B-rep join failed')
            return joined[0]
        finally:
            first.Dispose()
            second.Dispose()
    if kind == 'brep_cylinder':
        return G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.).ToBrep(True, True)
    if kind in ['brep_closed', 'brep_periodic_u', 'brep_periodic_v']:
        surface = create_source(dict(brep_closed='surface_closed',
            brep_periodic_u='surface_periodic_u', brep_periodic_v='surface_periodic_v')[kind], host)
        try:
            return surface.ToBrep()
        finally:
            surface.Dispose()
    if kind == 'brep_box':
        return G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,0.), G.Point3d(4.,3.,2.)))
    if kind == 'brep_surface':
        surface = create_source('surface', host)
        try:
            return surface.ToBrep()
        finally:
            surface.Dispose()
    if kind == 'brep_trimmed':
        boundary = G.PolylineCurve([G.Point3d(0.,0.,0.), G.Point3d(4.,0.,0.),
                                   G.Point3d(1.,3.,0.), G.Point3d(0.,0.,0.)])
        try:
            return G.Brep.CreateTrimmedPlane(G.Plane.WorldXY, boundary)
        finally:
            boundary.Dispose()
    if kind in ['mesh_strip', 'mesh_unwelded']:
        mesh = G.Mesh()
        mesh.Vertices.UseDoublePrecisionVertices = True
        points = [[0.,0.,0.],[2.,0.,0.],[4.,0.,1.],[0.,3.,0.],[2.,3.,0.],[4.,3.,1.]]
        faces = [[0,1,4,3],[1,2,5,4]]
        if kind == 'mesh_unwelded':
            points = [points[i] for f in faces for i in f]
            faces = [[0,1,2,3],[4,5,6,7]]
        for p in points:
            mesh.Vertices.Add(G.Point3d(*p))
        for f in faces:
            mesh.Faces.AddFace(*f)
        return mesh
    if kind.startswith('subd_'):
        return subd_holdout(kind, host)
    if kind != 'subd':
        return create_source(kind, host)
    mesh = G.Mesh()
    try:
        for p in [[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],
                  [0.,0.,2.],[4.,0.,2.],[4.,3.,2.],[0.,3.,2.]]:
            mesh.Vertices.Add(G.Point3d(*p))
        for f in [[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]]:
            mesh.Faces.AddFace(*f)
        result = G.SubD.CreateFromMesh(mesh)
        if result is None:
            raise ValueError('cannot construct ExtractPt SubD source')
        return result
    finally:
        mesh.Dispose()


def subd_holdout(kind, host):
    """Independent irregular, regular, boundary, and tagged edge controls."""
    G = host['Rhino'].Geometry
    if kind in ['subd_irregular', 'subd_creased']:
        subd = create('subd', host)
        if kind == 'subd_irregular':
            vertices, vertex = [], subd.Vertices.First
            while vertex is not None:
                vertices.append(vertex)
                vertex = vertex.Next
            for i,v in enumerate(vertices):
                p = v.ControlNetPoint
                v.ControlNetPoint = G.Point3d(p.X+0.13*i*i, p.Y+0.07*i, p.Z-0.21*(i%3))
        else:
            for edge in subd.Edges:
                if int(edge.Id) % 3 == 1:
                    edge.Tag = G.SubDEdgeTag.Crease
        subd.UpdateAllTagsAndSectorCoefficients()
        return subd
    mesh = G.Mesh()
    try:
        if kind == 'subd_grid':
            points = [[float(x), float(y), 0.17*x*y+0.11*(x%2)*(y%2)] for y in range(4) for x in range(4)]
            faces = [[4*y+x,4*y+x+1,4*y+x+5,4*y+x+4] for y in range(3) for x in range(3)]
        elif kind == 'subd_strip':
            points = [[0.,0.,0.],[2.,0.,0.],[4.,0.,1.],[0.,3.,0.],[2.,3.,0.],[4.,3.,1.]]
            faces = [[0,1,4,3],[1,2,5,4]]
        elif kind == 'subd_triangles':
            points = [[0.,0.,3.],[0.,0.,-2.],[2.,0.,0.],[0.,3.,0.],[-4.,0.,0.],[0.,-2.,0.]]
            faces = [[0,2,3],[0,3,4],[0,4,5],[0,5,2],[1,3,2],[1,4,3],[1,5,4],[1,2,5]]
        else:
            raise ValueError('unknown owned SubD source')
        for p in points:
            mesh.Vertices.Add(G.Point3d(*p))
        for f in faces:
            mesh.Faces.AddFace(*f)
        subd = G.SubD.CreateFromMesh(mesh)
        if subd is None:
            raise ValueError('could not create owned SubD holdout')
        return subd
    finally:
        mesh.Dispose()
