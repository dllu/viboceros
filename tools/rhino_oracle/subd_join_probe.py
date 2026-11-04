"""Independent public SDK joins of owned SubDs; sources stay unchanged."""
if __package__:
    from .subd_join_cases import case,validate
else:
    from subd_join_cases import case,validate


def source(record,host):
    G=host['Rhino'].Geometry
    mesh=G.Mesh();mesh.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in record['points']:mesh.Vertices.Add(G.Point3d(*p))
        for f in record['faces']:mesh.Faces.AddFace(*f)
        for n in record.get('ngons',[]):mesh.Ngons.AddNgon(G.MeshNgon.Create(host['System'].Array[host['System'].UInt32](n['vertices']),host['System'].Array[host['System'].UInt32](n['faces'])))
        mesh.Normals.ComputeNormals()
        subd=G.SubD.CreateFromMesh(mesh)
    finally:mesh.Dispose()
    if subd is None:raise ValueError('cannot create owned SubD join source')
    vertices=[];vertex=subd.Vertices.First
    while vertex is not None:vertices.append(vertex);vertex=vertex.Next
    indices={int(v.Id):i for i,v in enumerate(vertices)}
    for i in record.get('corners',[]):vertices[i].Tag=G.SubDVertexTag.Corner
    for edge in subd.Edges:
        pair=sorted([indices[int(edge.VertexFrom.Id)],indices[int(edge.VertexTo.Id)]])
        if pair in record.get('creases',[]):edge.Tag=G.SubDEdgeTag.Crease
    subd.UpdateAllTagsAndSectorCoefficients()
    return subd


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('SubD join requires an empty owned idle document')
    recipe=case(op['scene'],op['crease']);inputs=[];outputs=[]
    try:
        for record in recipe['sources']:inputs.append(source(record,host))
        before=[subd_value(s,host) for s in inputs]
        crc=[int(s.DataCRC(0)) for s in inputs]
        outputs=list(G.SubD.JoinSubDs(S.Array[G.SubD](inputs),recipe['tolerance'],recipe['crease']) or [])
        result=[subd_value(s,host) for s in outputs]
        after=[int(s.DataCRC(0)) for s in inputs]
        if after!=crc:raise ValueError('native SDK join edited a source')
        return dict(input=recipe,before=before,outputs=result,source_unchanged=True),0
    finally:
        for s in inputs+outputs:s.Dispose()
