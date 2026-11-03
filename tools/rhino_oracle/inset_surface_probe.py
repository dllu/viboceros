"""Owned public Brep.InsetFaces calls; no document edits or command macros."""
SCENES = ('rectangle', 'skew', 'concave', 'hole', 'circle', 'annulus',
          'warped', 'cylinder', 'box_one', 'box_pair')


def cases():
    return [dict(id=scene + ('_loose' if loose else '_tight') +
                 ('_crease' if crease else '_ring'), op='inset_surface_sdk',
                 scene=scene, distance=0.3, loose=loose, ignore_seams=True,
                 crease_corners=crease)
            for scene in SCENES for loose in (False, True) for crease in (False, True)] + [
        dict(id='cylinder_offset_seam', op='inset_surface_sdk', scene='cylinder',
             distance=0.3, loose=False, ignore_seams=False, crease_corners=True),
        dict(id='rectangle_large', op='inset_surface_sdk', scene='rectangle',
             distance=2.1, loose=False, ignore_seams=True, crease_corners=True)]


def validate(op):
    import math
    if not isinstance(op, dict):
        raise ValueError('invalid owned surface inset recipe')
    if (set(op) != set(('id', 'op', 'scene', 'distance', 'loose', 'ignore_seams', 'crease_corners'))
            or op['op'] != 'inset_surface_sdk' or op['scene'] not in SCENES
            or type(op['distance']) not in (int, float)
            or not 0 < op['distance'] <= 10 or math.isnan(op['distance'])
            or any(type(op[k]) is not bool for k in ('loose', 'ignore_seams', 'crease_corners'))):
        raise ValueError('invalid owned surface inset recipe')
    if op not in cases():
        raise ValueError('surface inset recipe differs from its owned case')


def source(scene, R):
    G = R.Geometry
    p = lambda x, y, z=0: G.Point3d(x, y, z)
    if scene in ('rectangle', 'skew'):
        b = G.PlaneSurface(G.Plane.WorldXY, G.Interval(0, 8), G.Interval(0, 6)).ToBrep()
        if scene == 'skew':
            b.Transform(G.Transform.Rotation(0.7, G.Vector3d(1, 2, 3), p(0, 0)))
            b.Transform(G.Transform.Translation(10, -20, 30))
        return b
    if scene in ('concave', 'hole', 'circle', 'annulus'):
        def poly(points):
            return G.PolylineCurve([p(x, y) for x, y in points + points[:1]])
        if scene == 'concave':
            curves = [poly([(0, 0), (8, 0), (8, 6), (5, 6), (5, 3), (3, 3), (3, 6), (0, 6)])]
        elif scene == 'hole':
            curves = [poly([(0, 0), (8, 0), (8, 6), (0, 6)]),
                      poly([(2, 2), (2, 4), (4, 4), (4, 2)])]
        else:
            curves = [G.Circle(G.Plane.WorldXY, 5).ToNurbsCurve()]
            if scene == 'annulus': curves.append(G.Circle(G.Plane.WorldXY, 2).ToNurbsCurve())
        try:
            outputs = G.Brep.CreatePlanarBreps(curves, 1e-6)
            if outputs is None or len(outputs) != 1: raise ValueError('planar source failed')
            return outputs[0]
        finally:
            for c in curves: c.Dispose()
    if scene == 'warped':
        return G.Brep.CreateFromCornerPoints(p(0, 0), p(8, 0), p(8, 6, 2), p(0, 6), 1e-6)
    if scene == 'cylinder':
        return G.Cylinder(G.Circle(G.Plane.WorldXY, 5), 6).ToBrep(False, False)
    return G.BoundingBox(p(0, 0), p(8, 6, 4)).ToBrep()


def run(op, host, iterations, tolerance):
    validate(op)
    return run_source(op, host, iterations, tolerance, source(op['scene'], host['Rhino']))


def run_source(op, host, iterations, tolerance, b):
    R, S = host['Rhino'], host['System']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    output = None
    def archive(geometry):
        model = R.FileIO.File3dm()
        try:
            model.Objects.AddBrep(geometry)
            return [int(v) for v in model.ToByteArray()]
        finally: model.Dispose()
    def record(geometry):
        return dict(valid=bool(geometry.IsValid), solid=bool(geometry.IsSolid),
                    geometry=host['_interchange_brep_record'](geometry), archive=archive(geometry))
    try:
        if not b.IsValid: raise ValueError('invalid owned source')
        before = record(b)
        crc = int(b.DataCRC(0))
        indices = [0, 1] if op['scene'] == 'box_pair' else [0]
        output = b.InsetFaces(S.Array[S.Int32](indices), op['distance'], op['loose'],
                             op['ignore_seams'], op['crease_corners'],
                             tolerance['absolute'], tolerance['angular'])
        if int(b.DataCRC(0)) != crc: raise ValueError('InsetFaces edited its source')
        return dict(before=before, faces=indices, source_unchanged=True,
                    output=record(output) if output is not None else None), 0
    finally:
        if output is not None: output.Dispose()
        b.Dispose()
