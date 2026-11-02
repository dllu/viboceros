"""Owned native curvature commands, public settings and analysis mesh samples."""
import math
import os
import re

SOURCES=('warped','holed','box','sphere','cylinder','cone','torus','subd_plane','subd_box')
STYLES=('Gaussian','Mean','MinRadius','MaxRadius')
STEPS=('CurvatureAnalysis','CurvatureAnalysisOff','DraftAngleAnalysis','DraftAngleAnalysisOff','Hide','Show','Delete','Undo','Redo','MaxRange','AutoRange')

def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','source','style','range','preselect','steps'}
            or op['op']!='curvature_analysis' or op['source'] not in SOURCES or op['style'] not in STYLES
            or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or type(op['preselect']) is not bool or not isinstance(op['range'],list) or len(op['range'])!=2
            or any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in op['range'])
            or not isinstance(op['steps'],list) or not 1<=len(op['steps'])<=24 or any(s not in STEPS for s in op['steps'])):
        raise ValueError('invalid curvature analysis recipe')

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    settings=R.ApplicationSettings.CurvatureAnalysisSettings
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('curvature capture requires empty owned idle document')
    previous=settings.GetCurrentState();model=None;source=None;key=S.Guid.Empty;crc=None
    # The public RhinoCommon member contains this spelling in Rhino 8.
    mode=R.Display.VisualAnalysisMode.Find(R.Display.VisualAnalysisMode.RhinoCurvatureColorAnalyisModeId)
    draft=R.Display.VisualAnalysisMode.Find(R.Display.VisualAnalysisMode.RhinoDraftAngleAnalysisModeId)
    def state(s):
        return dict(style=str(s.Style),ranges=[[getattr(s,k).T0,getattr(s,k).T1] for k in ('GaussRange','MeanRange','MinRadiusRange','MaxRadiusRange')])
    def configure():
        s=settings.GetCurrentState();s.Style=getattr(settings.CurvatureStyle,op['style'])
        setattr(s,dict(Gaussian='GaussRange',Mean='MeanRange',MinRadius='MinRadiusRange',MaxRadius='MaxRadiusRange')[op['style']],G.Interval(*op['range']))
        settings.UpdateFromState(s)
    def snapshot():
        obj=doc.Objects.FindId(key)
        if obj is None:return dict(exists=False,selected=False,hidden=False,analysis=False,draft=False)
        if int(obj.Geometry.DataCRC(0))!=crc:raise ValueError('analysis changed source geometry')
        return dict(exists=True,selected=bool(obj.IsSelected(False)),hidden=bool(obj.IsHidden),analysis=bool(obj.InVisualAnalysisMode(mode)),draft=bool(obj.InVisualAnalysisMode(draft)))
    def samples():
        obj=doc.Objects.FindId(key)
        if obj is None or not obj.InVisualAnalysisMode(mode):return []
        result=[]
        if not isinstance(obj.Geometry,G.Brep):return []
        for face,m in zip(obj.Geometry.Faces,obj.GetMeshes(G.MeshType.Analysis) or []):
            if m.VertexColors.Count!=m.Vertices.Count:continue
            count=m.Vertices.Count;rows=[]
            indices=sorted(set([i*count//min(count,128) for i in range(min(count,128))]+([count-1] if count else [])))
            for i in indices:
                p=m.Vertices.Point3dAt(i);ok,u,v=face.ClosestPoint(p)
                if not ok:raise ValueError('native analysis sample projection failed')
                k=face.CurvatureAt(u,v)
                if k is None:continue
                c=m.VertexColors[i]
                rows.append(dict(index=int(i),uv=[float(u),float(v)],principal=[float(k.Kappa(0)),float(k.Kappa(1))],color=[int(c.R),int(c.G),int(c.B),int(c.A)]))
            result.append(dict(vertex_count=int(count),samples=rows))
        return result
    try:
        for command in ('CurvatureAnalysisOff','DraftAngleAnalysisOff'):R.RhinoApp.RunScript('_'+command,False)
        if op['source'].startswith('subd_'):
            mesh=G.Mesh()
            if op['source']=='subd_plane':
                for p in [(0,0,0),(3,0,0),(3,3,0),(0,3,0)]:mesh.Vertices.Add(*p)
                mesh.Faces.AddFace(0,1,2,3)
            else:
                for p in [(-2,-2,-2),(2,-2,-2),(2,2,-2),(-2,2,-2),(-2,-2,2),(2,-2,2),(2,2,2),(-2,2,2)]:mesh.Vertices.Add(*p)
                for f in [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]:mesh.Faces.AddFace(*f)
            source=G.SubD.CreateFromMesh(mesh);mesh.Dispose();key=doc.Objects.Add(source)
        else:
            model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','brep_direction',op['source']+'.3dm'))
            if model is None or model.Objects.Count!=1:raise ValueError('shared curvature source unavailable')
            source=list(model.Objects)[0].Geometry.DuplicateBrep()
            key=doc.Objects.AddBrep(source,None,None,False,False)
        if key==S.Guid.Empty:raise ValueError('curvature source insertion failed')
        crc=int(doc.Objects.FindId(key).Geometry.DataCRC(0))
        if op['preselect']:doc.Objects.Select(key)
        configure();doc.ClearUndoRecords(True);states=[snapshot()];traces=[]
        for step in op['steps']:
            select='' if any(o.IsSelected(False) for o in doc.Objects) else ' _SelID '+str(key)+' _Enter'
            macro=('_'+step+select if step in ('CurvatureAnalysis','DraftAngleAnalysis','Hide') else ('_SelID '+str(key)+' _Delete' if step=='Delete' else '_'+step))
            if step in ('MaxRange','AutoRange'):
                from curvature_analysis_controls import click_range
                click_range(step);success=True;events=[]
            else:success,unused,events=observe_command(R.Commands.Command,step,lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[],True)
            if step=='CurvatureAnalysis':configure()
            doc.Views.Redraw();R.RhinoApp.Wait()
            states.append(snapshot());traces.append(dict(command=step,success=success,events=[dict(name=e['name'],result=e['result']) for e in events],settings=state(settings.GetCurrentState()),meshes=samples()))
        return dict(defaults=state(settings.GetDefaultState()),states=states,traces=traces),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for command in ('CurvatureAnalysisOff','DraftAngleAnalysisOff'):R.RhinoApp.RunScript('_'+command,False)
        settings.UpdateFromState(previous)
        if key!=S.Guid.Empty:doc.Objects.Delete(key,True)
        if source is not None:source.Dispose()
        if model is not None:model.Dispose()
