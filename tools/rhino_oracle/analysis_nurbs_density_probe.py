"""Public native analysis density readback, meshing and archive rewriting."""
import os

ARCHIVES = ('density0','density25','density50','density80','density100')


def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','archive'}
            or op['op'] != 'analysis_nurbs_density' or op['archive'] not in ARCHIVES
            or op['id'] != op['archive']):
        raise ValueError('fixed analysis density archive required')


def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];G=R.Geometry
    active=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(active.Objects) or R.Commands.Command.InCommand():
        raise ValueError('analysis density capture requires empty owned idle document')
    path=os.path.join(os.path.dirname(__file__),'fixtures','analysis_nurbs_settings',op['archive']+'.3dm')
    doc=R.RhinoDoc.OpenHeadless(path)
    if doc is None:raise ValueError('native analysis archive load failed')
    mp=None;model=None
    try:
        mp=doc.GetAnalysisMeshingParameters()
        meshes=[]
        for source in ('plane','sphere','cylinder'):
            if source=='plane':brep=G.Brep.CreateFromCornerPoints(G.Point3d.Origin,G.Point3d(3,0,0),G.Point3d(3,2,0),G.Point3d(0,2,0),1e-8)
            elif source=='sphere':brep=G.Sphere(G.Point3d.Origin,2).ToBrep()
            else:brep=G.Cylinder(G.Circle(G.Plane.WorldXY,2),3).ToBrep(False,False)
            result=[]
            try:
                result=list(G.Mesh.CreateFromBrep(brep,mp) or [])
                if not result:raise ValueError('native analysis density mesh failed')
                meshes.append(dict(source=source,faces=sum(int(m.Faces.Count) for m in result),
                                   vertices=sum(int(m.Vertices.Count) for m in result)))
            finally:
                for mesh in result:mesh.Dispose()
                brep.Dispose()
        model=R.FileIO.File3dm.Read(path)
        if model is None:raise ValueError('native archive reread failed')
        encoded=model.ToByteArray()
        if encoded is None:raise ValueError('native archive rewrite failed')
        if R.RhinoDoc.ActiveDoc.RuntimeSerialNumber!=active.RuntimeSerialNumber or list(active.Objects):
            raise ValueError('headless reads changed the active document')
        subd=mp.SubDDisplayParameters();empty=G.SubD()
        try:settings=dict(density=float(mp.RelativeTolerance),subd_level=int(subd.DisplayDensity(empty)),subd_adaptive=bool(subd.DisplayDensityIsAdaptive))
        finally:subd.Dispose();empty.Dispose()
        return dict(settings=settings,meshes=meshes,archive_hex=''.join(format(int(b),'02x') for b in encoded)),0
    finally:
        if mp is not None:mp.Dispose()
        if model is not None:model.Dispose()
        doc.Dispose()
