"""Public limit samples, distinct from display-mesher approximations, and 3DM settings."""
import math
import os

ARCHIVES = ('level0-adaptive', 'level0-absolute', 'level2-adaptive',
            'level3-absolute', 'level5-adaptive', 'level6-absolute')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id'}
            or op['op'] != 'analysis_subd_details' or op['id'] != 'details'):
        raise ValueError('invalid analysis SubD details recipe')


def source(G, name):
    mesh = G.Mesh()
    if name == 'box':
        points = [(-2,-2,-2),(2,-2,-2),(2,2,-2),(-2,2,-2),(-2,-2,2),(2,-2,2),(2,2,2),(-2,2,2)]
        faces = [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]
    elif name == 'plane':
        points = [(0,0,0),(3,0,0),(3,2,0),(0,2,0)]; faces = [(0,1,2,3)]
    else:
        n = 3 if name == 'triangle' else 5
        points = [(math.cos(2*math.pi*i/n), math.sin(2*math.pi*i/n), 0) for i in range(n)]
        faces = [(0,i,i+1) for i in range(1,n-1)]
    try:
        for p in points: mesh.Vertices.Add(*p)
        for f in faces: mesh.Faces.AddFace(*f)
        if name == 'pentagon': mesh.Ngons.AddNgon(G.MeshNgon.Create([0,1,2,3,4],[0,1,2]))
        return G.SubD.CreateFromMesh(mesh)
    finally: mesh.Dispose()


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']; G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('analysis details capture requires empty owned idle document')
    xyz = lambda p: [float(p.X),float(p.Y),float(p.Z)]
    limits = []
    for name in ('plane','box','triangle','pentagon'):
        for level in range(6):
            subd = source(G, name)
            try:
                if level and not subd.Subdivide(level): raise ValueError('native subdivision failed')
                points = []; vertex = subd.Vertices.First
                while vertex is not None:
                    points.append(xyz(vertex.SurfacePoint())); vertex = vertex.Next
                indices = sorted(set([i*len(points)//min(64,len(points)) for i in range(min(64,len(points)))]+[len(points)-1]))
                limits.append(dict(source=name,level=level,vertices=len(points),faces=int(subd.Faces.Count),samples=[points[i] for i in indices]))
            finally: subd.Dispose()
    archives = []
    empty = G.SubD()
    try:
        for name in ARCHIVES:
            path = os.path.join(os.path.dirname(__file__),'fixtures','analysis_subd_settings',name+'.3dm')
            loaded = R.RhinoDoc.OpenHeadless(path)
            if loaded is None: raise ValueError('native headless archive load failed: '+name)
            try:
                mp = loaded.GetAnalysisMeshingParameters()
                try:
                    parameters = mp.SubDDisplayParameters()
                    try:
                        archives.append(dict(name=name,adaptive=bool(parameters.DisplayDensityIsAdaptive),
                                             effective_empty=int(parameters.DisplayDensity(empty))))
                    finally: parameters.Dispose()
                finally: mp.Dispose()
            finally: loaded.Dispose()
        if R.RhinoDoc.ActiveDoc.RuntimeSerialNumber != doc.RuntimeSerialNumber or list(doc.Objects):
            raise ValueError('headless settings reads changed the active document')
        return dict(limits=limits,archives=archives),0
    finally: empty.Dispose()
