"""Bounded public RhinoCommon meshing parameters and owned NURBS/B-rep outputs."""
SOURCES = ('plane', 'saddle', 'bump', 'oscillating', 'sphere', 'cylinder')
PROFILES = ('density0', 'density25', 'density50', 'density80', 'density100',
            'analysis', 'chord01', 'angle10', 'maxedge05')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op','id','source','profile'}
            or op['op'] != 'nurbs_meshing' or op['source'] not in SOURCES
            or op['profile'] not in PROFILES or op['id'] != op['source']+'-'+op['profile']):
        raise ValueError('fixed NURBS meshing recipe required')


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']; G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('NURBS meshing capture requires empty owned idle document')
    xyz = lambda p: [float(p.X),float(p.Y),float(p.Z)]
    name = op['source']
    if name in ('plane','saddle'):
        points = [(0,0,0),(3,0,1 if name=='saddle' else 0),(3,2,0),(0,2,2 if name=='saddle' else 0)]
        brep = G.Brep.CreateFromCornerPoints(*([G.Point3d(*p) for p in points]+[1e-8]))
    elif name in ('bump','oscillating'):
        degree = 4 if name=='bump' else 5
        surface = G.NurbsSurface.Create(3, False, degree+1, degree+1, degree+1, degree+1)
        for i in range(2*degree):
            surface.KnotsU[i] = 0. if i<degree else 1.
            surface.KnotsV[i] = 0. if i<degree else 1.
        for j in range(degree+1):
            for i in range(degree+1):
                z = (4. if i==2 and j==2 else 0.) if name=='bump' else (
                    0. if i in (0,degree) else (2. if i%2 else -2.))
                surface.Points.SetPoint(i,j,G.Point3d(3.*i/degree,2.*j/degree,z))
        brep=surface.ToBrep();surface.Dispose()
    elif name=='sphere': brep=G.Sphere(G.Point3d.Origin,2).ToBrep()
    else: brep=G.Cylinder(G.Circle(G.Plane.WorldXY,2),3).ToBrep(False,False)
    profile=op['profile']
    if profile.startswith('density'): mp=G.MeshingParameters(int(profile[7:])/100.)
    else:
        mp=G.MeshingParameters.DefaultAnalysisMesh
        if profile!='analysis':
            mp.RelativeTolerance=0.;mp.GridAngle=0.;mp.RefineAngle=0.;mp.GridMinCount=0
            if profile=='chord01':mp.Tolerance=.01
            elif profile=='angle10':mp.RefineAngleInDegrees=10.
            else:mp.MaximumEdgeLength=.5
    meshes=[]
    try:
        parameters={key:getattr(mp,key) for key in ('RelativeTolerance','Tolerance','GridMinCount',
            'GridAngle','RefineAngle','RefineGrid','SimplePlanes','MinimumEdgeLength','MaximumEdgeLength')}
        meshes=list(G.Mesh.CreateFromBrep(brep,mp) or [])
        if not meshes:raise ValueError('native NURBS meshing failed')
        records=[]
        for mesh in meshes:
            count=mesh.Vertices.Count
            indices=sorted(set([i*count//min(32,count) for i in range(min(32,count))]+[count-1]))
            records.append(dict(vertices=int(count),faces=int(mesh.Faces.Count),quads=int(mesh.Faces.QuadCount),
                samples=[xyz(mesh.Vertices.Point3dAt(i)) for i in indices]))
        return dict(parameters=parameters,meshes=records),0
    finally:
        for mesh in meshes:mesh.Dispose()
        mp.Dispose();brep.Dispose()
