"""Fixed public mesh channel edits, isolated from the user's document."""
import json
import os

OPERATIONS = ['identity','translate','mirror','scale','shear','flatten','flip',
              'triangulate','delete_face','cull','unweld','weld','append_plain','append_self']
CASES = {source+'_'+op:dict(source=source,operation=op)
         for source in ['texture_cube','ngon'] for op in OPERATIONS}


def request():
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='mesh_channels_sdk',id=name,case=name) for name in CASES])


def validate(operation):
    try:
        valid = (isinstance(operation,dict) and set(operation)=={'op','id','case'}
                 and operation['op']=='mesh_channels_sdk' and operation['case'] in CASES
                 and operation['id']==operation['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed mesh channel operation required')


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1:
        raise ValueError('one mesh channel iteration required')
    R,S = host['Rhino'],host['System']
    G=R.Geometry
    with open(os.path.join(os.path.dirname(__file__),'fixtures','mesh_channel_assets.json')) as stream:
        assets=json.load(stream)
    case=CASES[operation['case']]
    model=R.FileIO.File3dm.FromByteArray(S.Convert.FromBase64String(assets[case['source']]['data']))
    try:
        meshes=[o.Geometry for o in model.Objects if isinstance(o.Geometry,G.Mesh)]
        if len(meshes)!=1:
            raise ValueError('one fixed mesh asset required')
        mesh=meshes[0].DuplicateMesh()
    finally:
        model.Dispose()

    def value(mesh):
        result=host['_polygon_mesh_value'](mesh)
        result.update(normals=[[float(n.X),float(n.Y),float(n.Z)] for n in mesh.Normals],
            uv=[[float(t.X),float(t.Y)] for t in mesh.TextureCoordinates],
            ngons=[dict(vertices=[int(i) for i in g.BoundaryVertexIndexList()],faces=[int(i) for i in g.FaceIndexList()]) for g in mesh.Ngons],
            valid=bool(mesh.IsValid))
        return result

    try:
        before=value(mesh)
        op=case['operation']
        success=True
        if op in ['identity','translate','mirror','scale','shear','flatten']:
            transform=G.Transform.Identity
            if op=='translate':transform=G.Transform.Translation(4.,-3.,2.)
            elif op=='mirror':transform=G.Transform.Mirror(G.Plane.WorldYZ)
            elif op=='scale':transform=G.Transform.Scale(G.Plane.WorldXY,2.,3.,.5)
            elif op=='shear':transform.M01=.5
            elif op=='flatten':transform=G.Transform.Scale(G.Plane.WorldXY,1.,1.,0.)
            success=bool(mesh.Transform(transform))
        elif op=='flip':mesh.Flip(True,True,True)
        elif op=='triangulate':success=bool(mesh.Faces.ConvertQuadsToTriangles())
        elif op=='delete_face':mesh.Faces.DeleteFaces(S.Array[S.Int32]([0]))
        elif op=='cull':mesh.Vertices.CullUnused()
        elif op=='unweld':mesh.Unweld(0.,True)
        elif op=='weld':mesh.Weld(3.141592653589793)
        elif op=='append_plain':
            other=G.Mesh()
            try:
                for p in [(8.,0.,0.),(9.,0.,0.),(8.,1.,0.)]:other.Vertices.Add(*p)
                other.Faces.AddFace(0,1,2)
                mesh.Append(other)
            finally:other.Dispose()
        elif op=='append_self':
            other=mesh.DuplicateMesh()
            try:mesh.Append(other)
            finally:other.Dispose()
        return dict(input=case,before=before,after=value(mesh),success=success),0
    finally:
        mesh.Dispose()
