"""Read public camera/pointer context without deriving it from output geometry."""
def record(host, viewport):
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Cursor
    R, S = host['Rhino'], host['System']
    view = R.RhinoDoc.ActiveDoc.Views.ActiveView
    screen = Cursor.Position
    pixel = view.ScreenToClient(screen)
    ray = viewport.ClientToWorld(pixel)
    info = R.DocObjects.ViewportInfo(viewport)
    xyz = host['_xyz']
    try:
        perspective = bool(viewport.IsPerspectiveProjection)
        origin = info.CameraLocation if perspective else ray.From
        direction = ray.From - origin if perspective else ray.Direction
        return dict(pixel=[int(pixel.X),int(pixel.Y)],
                    viewport_line=dict(start=xyz(ray.From),end=xyz(ray.To)),
                    ray=dict(origin=xyz(origin),direction=xyz(direction),forward_only=perspective),
                    camera=dict(location=xyz(info.CameraLocation),direction=xyz(info.CameraDirection),up=xyz(info.CameraUp),
                                axes=[xyz(info.CameraX),xyz(info.CameraY),xyz(info.CameraZ)],
                                target=xyz(viewport.CameraTarget),projection='Perspective' if viewport.IsPerspectiveProjection else 'Parallel',
                                size=[int(viewport.Size.Width),int(viewport.Size.Height)],
                                frustum=[float(getattr(info,'Frustum'+k)) for k in ['Left','Right','Bottom','Top','Near','Far']]))
    finally:
        info.Dispose()
