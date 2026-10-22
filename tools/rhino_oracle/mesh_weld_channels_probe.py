"""Public Mesh.Weld channel and topology observations; no document edits."""
try:
    from .mesh_weld_channels_cases import CASES, FOLLOWUP, PREFIXES, PAIRS, validate
except (ImportError, ValueError):
    from mesh_weld_channels_cases import CASES, FOLLOWUP, PREFIXES, PAIRS, validate


def run(operation,host,iterations,tolerance):
    validate(operation)
    if iterations!=1: raise ValueError('one mesh welding iteration required')
    R,S=host['Rhino'],host['System']
    case=(PREFIXES if operation['op']=='mesh_weld_channels_prefixes' else FOLLOWUP if operation['op']=='mesh_weld_channels_followup' else dict(CASES,**PAIRS))[operation['case']]
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
        if case.get('unit_normals'):
            for i in range(mesh.Vertices.Count):
                n=R.Geometry.Vector3f(1.,0.,0.) if case['unit_normals']=='x' else R.Geometry.Vector3f(0.,0.,1. if i<3 else -1.)
                mesh.Normals.SetNormal(i,n)
        if case['channels']=='all':
            for i in range(mesh.Vertices.Count): mesh.VertexColors.Add(S.Drawing.Color.FromArgb((17*i)%256,(31*i)%256,(47*i)%256))
        def snapshot():
            value=host['_polygon_mesh_value'](mesh)
            value.update(uv=[[float(t.X),float(t.Y)] for t in mesh.TextureCoordinates],
                         normals=[[float(n.X),float(n.Y),float(n.Z)] for n in mesh.Normals],
                         colors=[[int(c.R),int(c.G),int(c.B)] for c in mesh.VertexColors],
                         ngons=[dict(vertices=[int(i) for i in g.BoundaryVertexIndexList()],faces=[int(i) for i in g.FaceIndexList()]) for g in mesh.Ngons],
                         valid=bool(mesh.IsValid))
            value['topology_vertices']=[list(mesh.TopologyVertices.MeshVertexIndices(i)) for i in range(mesh.TopologyVertices.Count)]
            value['topology_edges']=[]
            for i in range(mesh.TopologyEdges.Count):
                p=mesh.TopologyEdges.GetTopologyVertices(i)
                value['topology_edges'].append(dict(vertices=[int(p.I),int(p.J)],faces=[int(f) for f in mesh.TopologyEdges.GetConnectedFaces(i)]))
            return value
        before=snapshot()
        mesh.Weld(case['angle'])
        after=snapshot()
        return dict(input=case,before=before,after=after),0
    finally: mesh.Dispose()
