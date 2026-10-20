"""Read owned Viboceros exports through Rhino and check both geometric directions."""
import os
try:
    from . import morph_fit_benchmark_probe as source
except (ImportError, ValueError):
    import morph_fit_benchmark_probe as source


def validate(op):
    if not isinstance(op, dict) or set(op) != {'op', 'id', 'shape', 'axis', 'artifact_path'} or op['op'] != 'morph_fit_readback':
        raise ValueError('invalid deformation readback')
    source.validate(dict(op='morph_fit_benchmark', id=op['id'], shape=op['shape'], axis=op['axis']))
    path = op['artifact_path']
    if not isinstance(path, str) or os.path.basename(path) != op['id']+'.3dm' or not os.path.isabs(path):
        raise ValueError('owned named 3dm input required')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('one readback required')
    R = host['Rhino']
    model = R.FileIO.File3dm.Read(op['artifact_path'])
    if model is None or len(list(model.Objects)) != 1:
        raise ValueError('one exported object required')
    originals = source.source.source_geometry(op['shape'], host)
    a, b = [host['_point'](p) for p in source.AXES[op['axis']]]
    length = a.DistanceTo(b)
    n = [(y-x)/length for x, y in zip(host['_xyz'](a), host['_xyz'](b))]
    morph = R.Geometry.Morphs.StretchSpaceMorph(a, b, length*.5)
    try:
        output = list(model.Objects)[0].Geometry
        original = originals[0]
        valid, diagnostics = output.IsValidWithLog()
        def points(geometry):
            if isinstance(geometry, R.Geometry.Curve):
                return [geometry.PointAt(geometry.Domain.ParameterAt(i/256.)) for i in range(257)]
            result = []
            for face in geometry.Faces:
                for i in range(25):
                    for j in range(25):
                        u, v = face.Domain(0).ParameterAt(i/24.), face.Domain(1).ParameterAt(j/24.)
                        if face.IsPointOnFace(u, v) != R.Geometry.PointFaceRelation.Exterior:
                            result.append(face.PointAt(u, v))
            return result
        def nearest(geometry, p):
            if isinstance(geometry, R.Geometry.Curve):
                ok, t = geometry.ClosestPoint(p)
                if not ok:
                    raise ValueError('curve closest point failed')
                return geometry.PointAt(t)
            result = geometry.ClosestPoint(p)
            if not result.IsValid:
                raise ValueError('B-rep closest point failed')
            return result
        forward = [nearest(output, morph.MorphPoint(p)).DistanceTo(morph.MorphPoint(p)) for p in points(original)]
        def inverse(p):
            s = sum((x-y)*d for x, y, d in zip(host['_xyz'](p), host['_xyz'](a), n))
            if s <= 0.:
                old = s
            elif s >= length*.5:
                old = s+length*.5
            else:
                lo, hi = 0., length
                for _ in range(64):
                    mid = (lo+hi)*.5
                    t = mid/length
                    mapped = mid-length*.5*t*t*(3.-2.*t)
                    if mapped < s:
                        lo = mid
                    else:
                        hi = mid
                old = (lo+hi)*.5
            return host['_point']([x-d*(s-old) for x, d in zip(host['_xyz'](p), n)])
        reverse = []
        for p in points(output):
            before = inverse(p)
            reverse.append(nearest(original, before).DistanceTo(before))
        return dict(valid=bool(valid), diagnostics=str(diagnostics or ''),
                    geometry_type=output.GetType().Name,
                    faces=int(output.Faces.Count) if isinstance(output, R.Geometry.Brep) else None,
                    solid=bool(output.IsSolid) if isinstance(output, R.Geometry.Brep) else None,
                    forward_samples=len(forward), reverse_samples=len(reverse),
                    maximum_forward_distance=max(forward), maximum_inverse_distance=max(reverse)), 0
    finally:
        morph.Dispose()
        for g in originals:
            g.Dispose()
        model.Dispose()
