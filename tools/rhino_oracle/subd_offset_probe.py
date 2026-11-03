"""Public SDK offsets and source archives, with no document edits."""
if __package__:
    from .subd_offset_cases import scenes,validate
    from .subd_join_probe import source
else:
    from subd_offset_cases import scenes,validate
    from subd_join_probe import source


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    subd=source(scenes()[op['scene']],host);output=None;model=R.FileIO.File3dm()
    try:
        crc=int(subd.DataCRC(0));before=subd_value(subd,host)
        model.Objects.AddSubD(subd);archive=[int(b) for b in model.ToByteArray()]
        mesh=R.Geometry.Mesh.CreateFromSubDControlNet(subd)
        try:
            mesh.Normals.ComputeNormals()
            normals=[[float(n.X),float(n.Y),float(n.Z)]for n in mesh.Normals]
        finally:mesh.Dispose()
        output=subd.Offset(op['distance'],op['solid'])
        result=subd_value(output,host) if output is not None else None
        if int(subd.DataCRC(0))!=crc:raise ValueError('native SDK offset edited source')
        return dict(before=before,archive=archive,mesh_normals=normals,output=result,source_unchanged=True),0
    finally:
        if output is not None:output.Dispose()
        subd.Dispose();model.Dispose()
