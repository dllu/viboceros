"""Cached public camera frames across 3DM archives and live view restoration."""
import math
import re

PROJECTIONS = ('Parallel', 'Perspective', 'TwoPointPerspective')


def validate(op):
    def vector(value):
        return (isinstance(value, list) and len(value) == 3 and any(value)
                and all(type(x) in (int, float) and abs(x) <= 1e50
                        and not math.isnan(x) and not math.isinf(x) for x in value))
    if (not isinstance(op, dict) or set(op)-{'camera_samples'} != {'op', 'id', 'projection', 'direction', 'up', 'rotate'}
            or op['op'] != 'camera_archive' or op['projection'] not in PROJECTIONS
            or ('camera_samples' in op and type(op['camera_samples']) is not bool)
            or type(op['rotate']) is not bool or not isinstance(op['id'], str)
            or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or not vector(op['direction']) or not vector(op['up'])):
        raise ValueError('invalid camera archive recipe')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('camera archive capture requires an empty owned idle document')
    vp = doc.Views.ActiveView.ActiveViewport
    original = R.DocObjects.ViewportInfo(vp)
    old_target, old_name = vp.CameraTarget, vp.Name
    old_plane = vp.GetConstructionPlane()
    settings = R.ApplicationSettings.ViewSettings
    policy = (settings.DefinedViewSetCPlane, settings.DefinedViewSetProjection)
    info = record = model = copy = None
    index = -1

    def xyz(v):
        return [float(v.X), float(v.Y), float(v.Z)]

    def snapshot(v):
        return dict(direction=xyz(v.CameraDirection), up=xyz(v.CameraUp), location=xyz(v.CameraLocation),
                    frame=[xyz(v.CameraX), xyz(v.CameraY), xyz(v.CameraZ)],
                    projection='TwoPointPerspective' if v.IsTwoPointPerspectiveProjection else (
                        'Perspective' if v.IsPerspectiveProjection else 'Parallel'),
                    frustum=[float(getattr(v, 'Frustum'+k)) for k in ('Left', 'Right', 'Bottom', 'Top', 'Near', 'Far')])

    def live():
        current = R.DocObjects.ViewportInfo(vp)
        try:
            result = snapshot(current)
            result['target'] = xyz(vp.CameraTarget)
            result['viewport_size'] = [int(vp.Size.Width), int(vp.Size.Height)]
            result['points'] = []
            for coordinates in ([0, 0, 0], [11, 22, 25], [9, 18, 31]):
                point = G.Point3d(*coordinates)
                screen = vp.WorldToClient(point)
                result['points'].append(dict(world=coordinates, screen=[float(screen.X), float(screen.Y)]))
            if op.get('camera_samples'):
                for u, v, depth in ((0, 0, 10), (1, 2, 10), (-3, 1, 20)):
                    point = current.CameraLocation + current.CameraX*u + current.CameraY*v - current.CameraZ*depth
                    screen = vp.WorldToClient(point)
                    result['points'].append(dict(world=xyz(point), screen=[float(screen.X), float(screen.Y)]))
            return result
        finally:
            current.Dispose()

    try:
        settings.DefinedViewSetCPlane = True
        settings.DefinedViewSetProjection = True
        projection = R.Display.DefinedViewportProjection.Top if op['projection'] == 'Parallel' else R.Display.DefinedViewportProjection.Perspective
        vp.SetProjection(projection, 'Owned archive camera', False)
        vp.SetCameraTarget(G.Point3d.Origin, False)
        info = R.DocObjects.ViewportInfo(vp)
        if op['projection'] == 'TwoPointPerspective':
            if not info.ChangeToTwoPointPerspectiveProjection(10, G.Vector3d.ZAxis, 50):
                raise ValueError('two-point setup failed')
        info.IsCameraDirectionLocked = False
        info.IsCameraUpLocked = False
        info.SetCameraLocation(G.Point3d(10, 20, 30))
        info.SetCameraDirection(G.Vector3d(*op['direction']))
        if not info.SetCameraUp(G.Vector3d(*op['up'])):
            raise ValueError('invalid final camera axes')
        if op['projection'] == 'TwoPointPerspective':
            info.IsCameraUpLocked = True
            if not info.SetCameraDirection(G.Vector3d(*op['direction'])):
                raise ValueError('locked-up frame failed')
        half_width = 1.5 * float(vp.Size.Width) / vp.Size.Height
        if not info.SetFrustum(-half_width, half_width, -1.5, 1.5, 1, 1000) or not vp.SetViewProjection(info, False):
            raise ValueError('camera installation failed')
        if op['rotate']:
            vp.MouseRotateAroundTarget(S.Drawing.Point(100, 100), S.Drawing.Point(112, 93))
        source = live()
        record = R.DocObjects.ViewInfo(vp)
        record.Name = 'Owned archive view'
        model = R.FileIO.File3dm()
        model.NamedViews.Add(record)
        encoded = model.ToByteArray()
        copy = R.FileIO.File3dm.FromByteArray(encoded)
        if copy is None or copy.NamedViews.Count != 1:
            raise ValueError('camera archive failed')
        archive = snapshot(copy.NamedViews[0].Viewport)
        index = doc.NamedViews.Add('OwnedArchive_'+op['id'], vp.Id)
        if index < 0:
            raise ValueError('named view save failed')
        vp.SetProjection(R.Display.DefinedViewportProjection.Top, 'Temporary Top', False)
        if not doc.NamedViews.Restore(index, vp):
            raise ValueError('named view restore failed')
        named = live()
        vp.SetProjection(R.Display.DefinedViewportProjection.Top, 'Temporary Top', False)
        if not vp.SetViewProjection(copy.NamedViews[0].Viewport, False):
            raise ValueError('archived projection installation failed')
        installed = live()
        return dict(source=source, archive=archive, named_restore=named,
                    installed=installed, archive_base64=S.Convert.ToBase64String(encoded)), 0
    finally:
        if index >= 0:
            doc.NamedViews.Delete(index)
        vp.SetViewProjection(original, False)
        vp.SetCameraTarget(old_target, False)
        vp.SetConstructionPlane(old_plane)
        vp.Name = old_name
        settings.DefinedViewSetCPlane, settings.DefinedViewSetProjection = policy
        for value in (copy, model, record, info, original):
            if value is not None:
                value.Dispose()
