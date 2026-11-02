"""Public camera vectors, orthonormal frames and in-memory 3DM round trips."""
import math
import re

PROJECTIONS=('Parallel','Perspective','TwoPointPerspective')
ACTIONS=('None','Zoom','WorldTop','Plan','MousePan','MouseZoom','PhysicalOrbit')


def validate(op):
    def vector(v):
        return isinstance(v,list) and len(v)==3 and any(v) and all(type(x) in (int,float) and not math.isnan(x) and not math.isinf(x) and abs(x)<=1e50 for x in v)
    if (not isinstance(op,dict) or set(op)-{'mouse_drag'}!={'op','id','projection','direction','up','action'}
            or op['op']!='camera_vectors' or op['projection'] not in PROJECTIONS or op['action'] not in ACTIONS
            or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or not vector(op['direction']) or not vector(op['up'])):
        raise ValueError('invalid camera vector recipe')
    if (op['action']=='PhysicalOrbit') != ('mouse_drag' in op) or ('mouse_drag' in op and (not isinstance(op['mouse_drag'],list) or len(op['mouse_drag'])!=2 or any(type(x) is not int or abs(x)>50 for x in op['mouse_drag']))):raise ValueError('invalid camera mouse input')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('camera capture requires empty owned document')
    vp=doc.Views.ActiveView.ActiveViewport
    original=R.DocObjects.ViewportInfo(vp);old_plane=vp.GetConstructionPlane();old_name=vp.Name;old_target=vp.CameraTarget
    info=None;model=None;copy=None;record=None
    xyz=lambda v:[float(v.X),float(v.Y),float(v.Z)]
    def snapshot(v):
        return dict(direction=xyz(v.CameraDirection),up=xyz(v.CameraUp),location=xyz(v.CameraLocation),
                    frame=[xyz(v.CameraX),xyz(v.CameraY),xyz(v.CameraZ)],
                    projection='TwoPointPerspective' if v.IsTwoPointPerspectiveProjection else ('Perspective' if v.IsPerspectiveProjection else 'Parallel'),
                    frustum=[float(getattr(v,'Frustum'+key)) for key in ('Left','Right','Bottom','Top','Near','Far')])
    def live():
        current=R.DocObjects.ViewportInfo(vp)
        try:
            result=snapshot(current);result['target']=xyz(vp.CameraTarget)
            return result
        finally:current.Dispose()
    try:
        projection=R.Display.DefinedViewportProjection.Top if op['projection']=='Parallel' else R.Display.DefinedViewportProjection.Perspective
        vp.SetProjection(projection,'Owned raw camera',False)
        vp.SetCameraTarget(G.Point3d.Origin,False)
        info=R.DocObjects.ViewportInfo(vp)
        if op['projection']=='TwoPointPerspective':
            if not info.ChangeToTwoPointPerspectiveProjection(10,G.Vector3d.ZAxis,50):raise ValueError('two-point setup failed')
        info.IsCameraDirectionLocked=False;info.IsCameraUpLocked=False
        info.SetCameraLocation(G.Point3d(10,20,30))
        info.SetCameraDirection(G.Vector3d(*op['direction']))
        if not info.SetCameraUp(G.Vector3d(*op['up'])):raise ValueError('invalid final camera axes')
        if op['projection']=='TwoPointPerspective':
            info.IsCameraUpLocked=True
            if not info.SetCameraDirection(G.Vector3d(*op['direction'])):raise ValueError('locked-up frame failed')
        if not info.SetFrustum(-2,2,-1.5,1.5,1,1000):raise ValueError('frustum setup failed')
        if not vp.SetViewProjection(info,False):raise ValueError('camera installation failed')
        before=live()
        record=R.DocObjects.ViewInfo(vp);record.Name='Owned vector camera'
        model=R.FileIO.File3dm();model.NamedViews.Add(record)
        copy=R.FileIO.File3dm.FromByteArray(model.ToByteArray())
        if copy is None or copy.NamedViews.Count!=1:raise ValueError('native camera archive failed')
        saved=snapshot(copy.NamedViews[0].Viewport)
        if op['action']=='PhysicalOrbit':
            bounds=doc.Views.ActiveView.ScreenRectangle
            x,y=int((bounds.Left+bounds.Right)/2),int((bounds.Top+bounds.Bottom)/2)
            host['_record_progress']('PICK %s %d %d'%(op['id'],x,y))
            R.RhinoApp.RunScript('_EvaluatePt _Pause',False)
            if list(doc.Objects):raise ValueError('camera gesture created model geometry')
        elif op['action'].startswith('Mouse'):
            a=S.Drawing.Point(100,100);b=S.Drawing.Point(112,93)
            method=dict(MousePan='MouseLateralDolly',MouseZoom='MouseInOutDolly')[op['action']]
            getattr(vp,method)(a,b)
        elif op['action']=='Zoom':
            vp.Magnify(2,False)
        elif op['action'] in ('WorldTop','Plan'):
            if not R.RhinoApp.RunScript('_SetView _World _Top' if op['action']=='WorldTop' else '_Plan',False):raise ValueError('native preset failed')
        return dict(before=before,roundtrip=saved,after=live()),0
    finally:
        vp.SetViewProjection(original,False);vp.SetCameraTarget(old_target,False);vp.SetConstructionPlane(old_plane);vp.Name=old_name
        for value in (record,copy,model,info,original):
            if value is not None:value.Dispose()
