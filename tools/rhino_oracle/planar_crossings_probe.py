"""Observe exact face membership alongside Rhino's generated mesh coverage."""
def details(brep,points,host):
    G=host['Rhino'].Geometry;face=brep.Faces[0];meshes=list(G.Mesh.CreateFromBrep(brep,G.MeshingParameters.Default)or[])
    try:
        stations=[]
        for xyz in points:
            p=G.Point3d(*xyz);ok,u,v=face.ClosestPoint(p)
            if not ok:raise ValueError('face parameter proposal failed')
            stations.append(dict(point=xyz,relation=str(face.IsPointOnFace(u,v,1e-7)),
                                 mesh_present=any(p.DistanceTo(mesh.ClosestPoint(p))<1e-6 for mesh in meshes)))
        area=0.
        for mesh in meshes:
            props=G.AreaMassProperties.Compute(mesh)
            if props is None:raise ValueError('mesh area unavailable')
            try:area+=float(props.Area)
            finally:props.Dispose()
        return dict(samples=stations,mesh_count=len(meshes),mesh_area=area,
                    meshes=[dict(vertices=[host['_xyz'](p)for p in mesh.Vertices],
                                 faces=[[int(f.A),int(f.B),int(f.C)]+([int(f.D)]if f.IsQuad else[])for f in mesh.Faces])for mesh in meshes])
    finally:
        for mesh in meshes:mesh.Dispose()
