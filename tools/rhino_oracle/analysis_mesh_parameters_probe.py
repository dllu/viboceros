"""Owned public analysis-mesh defaults and SubD density/geometry observations."""
import math


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id'}
            or op['op'] != 'analysis_mesh_parameters' or op['id'] != 'parameters'):
        raise ValueError('invalid analysis mesh parameters recipe')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('analysis settings capture requires empty owned idle document')
    xyz = lambda p: [float(p.X), float(p.Y), float(p.Z)]

    def parameters(mp):
        names = ('ComputeCurvature', 'RelativeTolerance', 'GridMinCount', 'GridAngle',
                 'GridAspectRatio', 'RefineGrid', 'RefineAngle', 'MinimumEdgeLength',
                 'MaximumEdgeLength', 'Tolerance', 'SimplePlanes', 'JaggedSeams')
        subd = mp.SubDDisplayParameters()
        empty = G.SubD()
        try:
            result = {name: getattr(mp, name) for name in names}
            result['subd_level'] = int(subd.DisplayDensity(empty))
            result['subd_adaptive'] = bool(subd.DisplayDensityIsAdaptive)
            return result
        finally:
            subd.Dispose(); empty.Dispose()

    def mesh_record(mesh):
        count = mesh.Vertices.Count
        indices = sorted(set([i * count // min(64, count) for i in range(min(64, count))]
                             + ([count - 1] if count else [])))
        return dict(vertices=int(count), faces=int(mesh.Faces.Count),
                    quads=int(mesh.Faces.QuadCount),
                    samples=[xyz(mesh.Vertices.Point3dAt(i)) for i in indices])

    old = doc.GetAnalysisMeshingParameters()
    default = G.MeshingParameters.DefaultAnalysisMesh
    try:
        mappings = []
        counts = (0, 1, 124, 125, 126, 500, 501, 2000, 2001, 8000, 8001, 32000, 32001, 128000, 128001)
        for level in range(7):
            mappings.append(dict(level=level, counts=[dict(faces=n, effective=int(
                G.SubDDisplayParameters.AbsoluteDisplayDensityFromSubDFaceCount(level, n))) for n in counts]))
        meshes = []
        for source in ('plane', 'box', 'triangle', 'pentagon'):
            mesh = G.Mesh()
            if source == 'box':
                points = [(-2,-2,-2),(2,-2,-2),(2,2,-2),(-2,2,-2),(-2,-2,2),(2,-2,2),(2,2,2),(-2,2,2)]
                faces = [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]
            elif source == 'plane':
                points = [(0,0,0),(3,0,0),(3,2,0),(0,2,0)]; faces = [(0,1,2,3)]
            else:
                n = 3 if source == 'triangle' else 5
                points = [(math.cos(2*math.pi*i/n), math.sin(2*math.pi*i/n), 0) for i in range(n)]
                faces = [(0, i, i+1) for i in range(1, n-1)]
            for p in points: mesh.Vertices.Add(*p)
            for f in faces: mesh.Faces.AddFace(*f)
            # Merge triangulated planar regions into an ngon for the pentagon.
            if source == 'pentagon':
                mesh.Ngons.AddNgon(G.MeshNgon.Create([0,1,2,3,4],[0,1,2]))
            subd = G.SubD.CreateFromMesh(mesh)
            mesh.Dispose()
            try:
                levels = []
                for level in range(1, 6):
                    output = G.Mesh.CreateFromSubD(subd, level)
                    try: levels.append(dict(level=level, mesh=mesh_record(output)))
                    finally: output.Dispose()
                key = doc.Objects.AddSubD(subd)
                doc.Objects.Select(key)
                success = bool(R.RhinoApp.RunScript('_ExtractAnalysisMesh', False))
                outputs = [o for o in doc.Objects if o.Id != key]
                if not success or len(outputs) != 1 or not isinstance(outputs[0].Geometry, G.Mesh):
                    raise ValueError('native default analysis extraction failed')
                meshes.append(dict(source=source, levels=levels, extraction=mesh_record(outputs[0].Geometry)))
                for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)
            finally: subd.Dispose()
        return dict(current=parameters(old), default=parameters(default),
                    adaptive_quad_maximum=int(G.SubDDisplayParameters.AdaptiveDisplayMeshQuadMaximum),
                    mappings=mappings, meshes=meshes), 0
    finally:
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)
        old.Dispose(); default.Dispose()
