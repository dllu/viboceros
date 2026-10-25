"""Compare the public surface G1 query with the public B-rep crease splitter."""
import math


def inspect(surface, angle, host):
    if isinstance(angle, bool) or not isinstance(angle, (float, int)) or not 0. < angle < math.pi:
        raise ValueError('invalid surface crease angle')
    R = host['Rhino']
    before = int(surface.DataCRC(0))
    breaks = []
    for axis in [0, 1]:
        domain = surface.Domain(axis)
        start = domain.T0
        values = []
        while True:
            found, t = surface.GetNextDiscontinuity(
                axis, R.Geometry.Continuity.G1_continuous, start, domain.T1,
                math.cos(angle), math.sqrt(2.220446049250313e-16))
            if not found: break
            if not start < t < domain.T1:
                raise ValueError('invalid native continuity traversal')
            values.append(float(t))
            start = t
        breaks.append(values)
    brep = surface.ToBrep()
    if brep is None: raise ValueError('could not construct crease B-rep')
    try:
        initial = brep.Faces.Count
        success = bool(brep.Faces.SplitKinkyFaces(float(angle), True))
        if not success or not brep.IsValid: raise ValueError('native crease splitting failed')
        faces = []
        for face in brep.Faces:
            record = host['_loft_surface_record'](face, dict(sample_geometry=True))
            faces.append((record, bool(face.OrientationIsReversed)))
        faces.sort(key=lambda pair: (pair[0]['domain_v'], pair[0]['domain_u']))
        if before != int(surface.DataCRC(0)): raise ValueError('crease probe mutated its source')
        return dict(initial_faces=int(initial), succeeded=success, valid=bool(brep.IsValid),
                    raw_breaks=breaks, surfaces=[f[0] for f in faces],
                    face_reversed=[f[1] for f in faces], vertices=int(brep.Vertices.Count),
                    edges=int(brep.Edges.Count), source_unchanged=True)
    finally:
        brep.Dispose()
