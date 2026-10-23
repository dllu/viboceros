"""Public Mesh.RebuildNormals and owned native RebuildMeshNormals commands."""
try:
    from .rebuild_normals_cases import SDK,COMMANDS,EXTRA_SDK,EXTRA_COMMANDS,validate
except (ImportError,ValueError):
    from rebuild_normals_cases import SDK,COMMANDS,EXTRA_SDK,EXTRA_COMMANDS,validate


def run(op,host,iterations,tolerance):
    validate(op)
    if op['op']=='rebuild_mesh_normals_command':
        from crease_probe import run_scene
        cases=COMMANDS if op['case']in COMMANDS else EXTRA_COMMANDS
        return run_scene(cases[op['case']],host,iterations,tolerance)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=SDK[op['case']]if op['case']in SDK else EXTRA_SDK[op['case']];m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in case['vertices']:m.Vertices.Add(*p)
        for f in case['faces']:m.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(255-a,r,g,b))
        for uv in case.get('uv',[]):m.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):m.Normals.Add(*n)
        for n in case.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        def value():
            v=snapshot(m,host,{'transparency':[]});v['face_normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in m.FaceNormals];return v
        before=value();m.RebuildNormals()
        return dict(input=case,before=before,after=value()),0
    finally:m.Dispose()
