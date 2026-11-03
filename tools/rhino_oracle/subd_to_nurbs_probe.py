"""Owned SubD-to-NURBS SDK controls for packing and vertex continuity."""
import math
if __package__:
    from .subd_join_probe import source as from_net
    from .subd_box_probe import subd_value
else:
    from subd_join_probe import source as from_net
    from subd_box_probe import subd_value


SCENES = ('quad_corners', 'quad_smooth', 'grid', 'box', 'pentagon', 'creased_box', 'torus')


def cases():
    return [dict(id=scene+'_'+str(packed)+'_'+str(vertex), op='subd_to_nurbs_sdk',
                 scene=scene, packed=packed, vertex=vertex)
            for scene in SCENES for packed in (False, True)
            for vertex in (range(5) if scene in ('box', 'pentagon') else (3,))]


def validate(op):
    if (not isinstance(op, dict) or op not in cases()
            or type(op.get('packed')) is not bool or type(op.get('vertex')) is not int):
        raise ValueError('invalid owned SubD to NURBS recipe')


def source(scene):
    if scene in ('quad_corners', 'quad_smooth'):
        return dict(points=[[0.,0.,0.],[4.,0.,1.],[4.,3.,2.],[0.,3.,-1.]],
                    faces=[[0,1,2,3]], corners=list(range(4)) if scene=='quad_corners' else [])
    if scene=='grid':
        return dict(points=[[float(x),float(y),0.15*x*y+0.07*x*x] for y in range(4) for x in range(5)],
                    faces=[[5*y+x,5*y+x+1,5*(y+1)+x+1,5*(y+1)+x] for y in range(3) for x in range(4)],
                    corners=[0,4,15,19])
    if scene in ('box', 'creased_box'):
        return dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],
                            [0.,0.,2.],[4.,0.,2.],[4.,3.,2.],[0.,3.,2.]],
                    faces=[[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]],
                    creases=[[0,1],[1,2],[2,3],[0,3]] if scene=='creased_box' else [])
    if scene=='pentagon':
        return dict(points=[[3*math.cos(i*(2*math.pi)/5),3*math.sin(i*(2*math.pi)/5),0.4*math.sin(2*i*(2*math.pi)/5)] for i in range(5)],
                    faces=[list(range(5))], corners=list(range(5)))
    if scene=='torus':
        points=[];faces=[]
        for j in range(4):
            v=j*(2*math.pi)/4
            for i in range(8):
                u=i*(2*math.pi)/8;points.append([(4+math.cos(v))*math.cos(u),(4+math.cos(v))*math.sin(u),math.sin(v)])
        for j in range(4):
            for i in range(8):faces.append([j*8+i,j*8+(i+1)%8,((j+1)%4)*8+(i+1)%8,((j+1)%4)*8+i])
        return dict(points=points, faces=faces)
    raise ValueError('unknown owned SubD source')


def run(op, host, iterations, tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    recipe=source(op['scene']);s=make_source(recipe,host);b=None;options=None
    try:
        before=subd_value(s,host);crc=int(s.DataCRC(0))
        options=G.SubDToBrepOptions(op['packed'],host['System'].Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,op['vertex']))
        b=s.ToBrep(options)
        if int(s.DataCRC(0))!=crc:raise ValueError('SubD to Brep edited its source')
        model=R.FileIO.File3dm()
        try:
            model.Objects.AddSubD(s)
            archive=[int(v) for v in model.ToByteArray()]
        finally:model.Dispose()
        edges=[]
        if not op['packed'] and op['vertex']==3:
            s.UpdateSurfaceMeshCache(False)
            for i,edge in enumerate(s.Edges):
                curves=[]
                for clamp in (False,True):
                    c=edge.ToNurbsCurve(clamp)
                    try:curves.append(None if c is None else host['_nurbs_curve_definition'](c))
                    finally:
                        if c is not None:c.Dispose()
                edges.append(dict(index=i,curves=curves))
        return dict(input=recipe,before=before,archive=archive,source_unchanged=int(s.DataCRC(0))==crc,
                    edges=edges,output=None if b is None else dict(valid=bool(b.IsValid),solid=bool(b.IsSolid),
                    geometry=host['_interchange_brep_record'](b))),0
    finally:
        if options is not None:options.Dispose()
        if b is not None:b.Dispose()
        s.Dispose()


def make_source(recipe,host):
    if all(len(f)<=4 for f in recipe['faces']):return from_net(recipe,host)
    G=host['Rhino'].Geometry;S=host['System'];mesh=G.Mesh()
    mesh.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in recipe['points']:mesh.Vertices.Add(G.Point3d(*p))
        for f in recipe['faces']:
            start=mesh.Faces.Count
            for i in range(1,len(f)-1):mesh.Faces.AddFace(f[0],f[i],f[i+1])
            mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](f),
                S.Array[S.UInt32](list(range(start,mesh.Faces.Count)))))
        mesh.Normals.ComputeNormals();subd=G.SubD.CreateFromMesh(mesh)
    finally:mesh.Dispose()
    if subd is None:raise ValueError('cannot create owned polygon SubD')
    vertex=subd.Vertices.First;i=0
    while vertex is not None:
        if i in recipe.get('corners',[]):vertex.Tag=G.SubDVertexTag.Corner
        vertex=vertex.Next;i+=1
    subd.UpdateAllTagsAndSectorCoefficients()
    return subd
