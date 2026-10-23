"""Public UnifyNormals count-only and editing overloads, plus owned commands."""
try:
    from .unify_normals_cases import SDK,DISCOVERY,EXTRA,HOLDOUTS,COMMANDS,SEEDS,FOLLOWUPS,validate
except (ImportError,ValueError):
    from unify_normals_cases import SDK,DISCOVERY,EXTRA,HOLDOUTS,COMMANDS,SEEDS,FOLLOWUPS,validate


def run(op,host,iterations,tolerance):
    validate(op)
    if op['op']=='unify_normals_command':
        from crease_probe import run_scene
        return run_scene(next(cases[op['case']]for cases in [DISCOVERY,COMMANDS,FOLLOWUPS]if op['case']in cases),host,iterations,tolerance)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.RhinoDoc.ActiveDoc.UndoRecordingIsActive:raise ValueError('owned empty idle document required')
    from mesh_channel_capture import snapshot
    case=next(cases[op['case']]for cases in [SDK,EXTRA,HOLDOUTS,SEEDS,FOLLOWUPS]if op['case']in cases);m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
    try:
        for p in case['vertices']:m.Vertices.Add(*p)
        for f in case['faces']:m.Faces.AddFace(*f)
        for r,g,b,a in case.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(255-a,r,g,b))
        for uv in case.get('uv',[]):m.TextureCoordinates.Add(*uv)
        for n in case.get('normals',[]):m.Normals.Add(*n)
        for n in case.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        if case.get('compute_face_normals'):m.FaceNormals.ComputeFaceNormals()
        def value():
            v=snapshot(m,host,{'transparency':[]});v['face_normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in m.FaceNormals];return v
        before=value();predicted=m.UnifyNormals(True);after_count=value();changed=m.UnifyNormals(False)if case.get('boolean_overload')else m.UnifyNormals();after=value();second=m.UnifyNormals(False)if case.get('boolean_overload')else m.UnifyNormals()
        return dict(input=case,before=before,predicted=int(predicted),after_count=after_count,changed=int(changed),after=after,second=int(second),after_second=value()),0
    finally:m.Dispose()
