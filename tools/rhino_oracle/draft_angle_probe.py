"""Public DraftAngleAnalysis command/settings and rendered mesh observations."""
import math
import os
import re

SOURCES=('mesh','panels','warped','holed','box','sphere')
STEPS=('DraftAngleAnalysis','DraftAngleAnalysisOff','Hide','Show','Delete','Undo','Redo','SelNone')

def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','range','up','steps','source','preselect','wires'} or op['op']!='draft_angle_analysis'
            or op['source'] not in SOURCES or type(op['preselect']) is not bool or type(op['wires']) is not bool
            or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or not isinstance(op['range'],list) or len(op['range'])!=2 or not isinstance(op['up'],list) or len(op['up'])!=3
            or any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in op['range']+op['up'])
            or not any(op['up']) or any(abs(x)>90 for x in op['range'])
            or not isinstance(op['steps'],list) or not 1 <= len(op['steps']) <= 24
            or any(x not in STEPS for x in op['steps'])):
        raise ValueError('invalid draft angle recipe')

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    settings=R.ApplicationSettings.DraftAngleAnalysisSettings
    from join_probe import observe_command
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('draft-angle capture requires an empty owned idle document')
    previous=settings.GetCurrentState();factory=settings.GetDefaultState()
    def state(s):
        return dict(range=[s.AngleRange.T0,s.AngleRange.T1],up=[s.UpDirection.X,s.UpDirection.Y,s.UpDirection.Z],wires=s.ShowIsoCurves)
    key=S.Guid.Empty;mesh=G.Mesh();panels=G.Brep();model=None;source=None;original_crc=None
    for i,angle in enumerate((-90,-45,-10,-5,-2.5,0,2.5,5,10,45,90)):
        a=math.radians(angle);start=mesh.Vertices.Count
        mesh.Vertices.Add(i*3,0,0);mesh.Vertices.Add(i*3,1,0);mesh.Vertices.Add(i*3-math.sin(a),0,math.cos(a))
        mesh.Faces.AddFace(start,start+1,start+2)
        face=G.Brep.CreateFromCornerPoints(G.Point3d(i*3,0,0),G.Point3d(i*3,1,0),G.Point3d(i*3-math.sin(a),0,math.cos(a)),1e-8)
        panels.Append(face);face.Dispose()
    mesh.Normals.ComputeNormals()
    mode=R.Display.VisualAnalysisMode.Find(R.Display.VisualAnalysisMode.RhinoDraftAngleAnalysisModeId)
    def snapshot():
        obj=doc.Objects.FindId(key)
        if obj is None:return dict(exists=False,selected=False,hidden=False,analysis=False)
        if int(obj.Geometry.DataCRC(0))!=original_crc:raise ValueError('analysis changed source geometry')
        return dict(exists=True,selected=bool(obj.IsSelected(False)),hidden=bool(obj.IsHidden),analysis=bool(obj.InVisualAnalysisMode(mode)))
    def colors():
        obj=doc.Objects.FindId(key)
        if obj is None:return []
        result=[]
        for m in obj.GetMeshes(G.MeshType.Analysis) or []:
            if m.VertexColors.Count!=m.Vertices.Count:continue
            count=m.Vertices.Count
            indices=sorted(set([i*count//min(count,256) for i in range(min(count,256))]+([count-1] if count else [])))
            result.append(dict(vertex_count=int(count),indices=indices,
                normals=[[float(m.Normals[i].X),float(m.Normals[i].Y),float(m.Normals[i].Z)] for i in indices],
                colors=[[int(m.VertexColors[i].R),int(m.VertexColors[i].G),int(m.VertexColors[i].B),int(m.VertexColors[i].A)] for i in indices]))
        return result
    try:
        R.RhinoApp.RunScript('_DraftAngleAnalysisOff',False)
        settings.AngleRange=G.Interval(*[math.radians(x) for x in op['range']]);settings.UpDirection=G.Vector3d(*op['up']);settings.ShowIsoCurves=op['wires']
        if op['source'] in ('mesh','panels'):source=mesh if op['source']=='mesh' else panels
        else:
            model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','brep_direction',op['source']+'.3dm'))
            if model is None or model.Objects.Count!=1:raise ValueError('shared draft-angle operand unavailable')
            source=list(model.Objects)[0].Geometry.DuplicateBrep()
        key=doc.Objects.AddMesh(source) if isinstance(source,G.Mesh) else doc.Objects.AddBrep(source,None,None,False,False)
        if key==S.Guid.Empty:raise ValueError('draft-angle insertion failed')
        original_crc=int(doc.Objects.FindId(key).Geometry.DataCRC(0))
        if op['preselect']:doc.Objects.Select(key)
        doc.ClearUndoRecords(True)
        states=[snapshot()];traces=[]
        for step in op['steps']:
            if step=='SelNone':doc.Objects.UnselectAll();success=True;events=[]
            else:
                select='' if any(o.IsSelected(False) for o in doc.Objects) else ' _SelID '+str(key)+' _Enter'
                macro=('_'+step+select if step in ('DraftAngleAnalysis','Hide') else ('_SelID '+str(key)+' _Delete' if step=='Delete' else '_'+step))
                success,unused,events=observe_command(R.Commands.Command,step,lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
            # Starting the command resets the pull direction to world Z.
            # Apply the public setting after entry, as a panel direction edit.
            if step=='DraftAngleAnalysis':settings.UpDirection=G.Vector3d(*op['up'])
            doc.Views.Redraw();R.RhinoApp.Wait()
            states.append(snapshot());traces.append(dict(command=step,success=success,events=[dict(name=e['name'],result=e['result']) for e in events],settings=state(settings.GetCurrentState()),meshes=colors()))
        return dict(defaults=state(factory),initial=state(previous),states=states,traces=traces),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        R.RhinoApp.RunScript('_DraftAngleAnalysisOff',False)
        settings.UpdateFromState(previous)
        if key!=S.Guid.Empty:doc.Objects.Delete(key,True)
        mesh.Dispose();panels.Dispose()
        if model is not None:source.Dispose();model.Dispose()
