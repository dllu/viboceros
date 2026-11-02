"""Owned Draft Angle panel controls and public pull-direction observations."""
import math
import os
import re

MODES=('WorldZ','CPlaneZ','View','TwoPoints','Curve')
LABELS=dict(WorldZ='World Z',CPlaneZ='CPlane Z',View='View based',TwoPoints='Two points',Curve='Curve or edge')
CURVES=('None','Line','Polyline','Arc','BoxEdge')


def validate(op):
    def vector(v):
        return isinstance(v,list) and len(v)==3 and all(type(x) in (int,float) and not math.isnan(x) and not math.isinf(x) for x in v)
    if (not isinstance(op,dict) or set(op)!={'op','id','mode','plane','view','points','curve','edge','reverse','cancel'}
            or op['op']!='draft_pull_direction' or op['mode'] not in MODES or op['curve'] not in CURVES
            or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or any(not vector(op[k]) or not any(op[k]) for k in ('plane','view'))
            or not isinstance(op['points'],list) or len(op['points'])!=2 or any(not vector(p) for p in op['points'])
            or type(op['reverse']) is not bool or type(op['cancel']) is not bool
            or type(op['edge']) is not int or not 0<=op['edge']<12
            or (op['cancel'] and op['mode'] not in ('TwoPoints','Curve'))
            or (op['reverse'] and op['curve'] not in ('Line','Polyline','Arc'))
            or ((op['mode']=='Curve') != (op['curve']!='None'))):
        raise ValueError('invalid draft pull-direction recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from draft_pull_controls import choose,click,prepare,panel
    from draft_pull_input import start
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('pull-direction capture requires empty owned document')
    settings=R.ApplicationSettings.DraftAngleAnalysisSettings
    previous=settings.GetCurrentState();key=S.Guid.Empty;target=S.Guid.Empty;mesh=G.Mesh();source=None;model=None;finish=None
    vp=doc.Views.ActiveView.ActiveViewport
    old_plane=vp.GetConstructionPlane();old_camera=R.DocObjects.ViewportInfo(vp)
    def up():
        v=settings.UpDirection
        return [float(v.X),float(v.Y),float(v.Z)]
    try:
        for p in [(0,0,0),(1,0,0),(0,1,0)]:mesh.Vertices.Add(*p)
        mesh.Faces.AddFace(0,1,2);mesh.Normals.ComputeNormals()
        key=doc.Objects.AddMesh(mesh);doc.Objects.Select(key)
        plane=G.Plane(G.Point3d.Origin,G.Vector3d(*op['plane']))
        vp.SetConstructionPlane(plane)
        vp.SetCameraDirection(G.Vector3d(*op['view']),True)
        if not R.RhinoApp.RunScript('_DraftAngleAnalysis',False):raise ValueError('draft entry failed')
        R.RhinoApp.Wait()
        prepare(LABELS[op['mode']])
        settings.UpDirection=G.Vector3d(9,8,7)
        if op['curve']!='None':
            if op['curve']=='Line':source=G.LineCurve(G.Point3d(*op['points'][0]),G.Point3d(*op['points'][1]))
            elif op['curve']=='Polyline':source=G.PolylineCurve([G.Point3d(0,0,0),G.Point3d(2,3,6),G.Point3d(4,3,6)])
            elif op['curve']=='Arc':source=G.ArcCurve(G.Arc(G.Plane.WorldXY,2,math.pi/2))
            else:
                model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','brep_direction','box.3dm'))
                if model is None or model.Objects.Count!=1:raise ValueError('owned edge reference missing')
                source=list(model.Objects)[0].Geometry.DuplicateBrep()
            if op['reverse']:
                if isinstance(source,G.Curve):source.Reverse()
                else:raise ValueError('Brep reversal is not a curve reversal')
            target=doc.Objects.AddBrep(source,None,None,False,False) if isinstance(source,G.Brep) else doc.Objects.AddCurve(source)
            reference_crc=int(doc.Objects.FindId(target).Geometry.DataCRC(0))
            if not op['cancel']:
                if op['curve']=='BoxEdge':
                    doc.Objects.UnselectAll()
                else:doc.Objects.Select(target)
        if op['curve']=='BoxEdge':
            vp.SetProjection(R.Display.DefinedViewportProjection.Perspective,'Owned pull edge',False)
            vp.ZoomExtents();doc.Views.ActiveView.Redraw();R.RhinoApp.Wait()
            panel(move_for_pick=True);R.RhinoApp.Wait()
        crc=int(doc.Objects.FindId(key).Geometry.DataCRC(0))
        finish=start(R,S,op,target) if op['mode'] in ('TwoPoints','Curve') else None
        button,choices=choose(LABELS[op['mode']]);R.RhinoApp.Wait();selected=up()
        inputs=finish() if finish is not None else dict(edge_pick_acknowledged=False,sent=[],errors=[]);finish=None
        if op['mode']=='Curve' and not op['cancel']:
            if op['curve']=='BoxEdge':doc.Objects.UnselectAll()
            else:doc.Objects.Select(target)
        finish=start(R,S,op,target) if op['mode'] in ('TwoPoints','Curve') else None
        click(button);R.RhinoApp.Wait()
        repeated=finish() if finish is not None else dict(edge_pick_acknowledged=False,sent=[],errors=[]);finish=None
        if op['curve']=='BoxEdge' and not op['cancel']:
            expected=doc.Objects.FindId(target).Geometry.Edges[op['edge']].TangentAtStart
            if (G.Vector3d(*up())-expected).Length>1e-12:raise ValueError('physical pick selected another edge: '+str(up())+' expected '+str(expected))
        if int(doc.Objects.FindId(key).Geometry.DataCRC(0))!=crc:raise ValueError('direction setting mutated source')
        if target!=S.Guid.Empty and int(doc.Objects.FindId(target).Geometry.DataCRC(0))!=reference_crc:raise ValueError('direction setting mutated reference')
        return dict(choices=choices,selected=selected,up=up(),input=inputs,
                    repeated=repeated,camera=[vp.CameraDirection.X,vp.CameraDirection.Y,vp.CameraDirection.Z]),0
    finally:
        if finish is not None:
            try:finish()
            except Exception:pass
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        R.RhinoApp.RunScript('_DraftAngleAnalysisOff',False)
        settings.UpdateFromState(previous)
        vp.SetConstructionPlane(old_plane);vp.SetViewProjection(old_camera,False)
        for identity in (target,key):
            if identity!=S.Guid.Empty:doc.Objects.Delete(identity,True)
        if source is not None:source.Dispose()
        if model is not None:model.Dispose()
        mesh.Dispose()
