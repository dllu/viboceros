"""Independent bidirectional raw-vertex samples of mesh attribute fields."""
import math
from .mesh_boolean_comparison import cross, dot, sub, triangles

CHANNELS = {'uv': (2, 1e-6), 'vertex_normals': (3, 1e-6), 'colors': (4, 1.)}
GEOMETRY_EPSILON = 1e-7


def validate(mesh):
    list(triangles([mesh]))
    for name, (width, _) in CHANNELS.items():
        if name not in mesh:
            raise ValueError('missing mesh vertex channel')
        values = mesh[name]
        if values is None:
            continue
        if (not isinstance(values, list) or len(values) != len(mesh['vertices']) or
                any(not isinstance(v, list) or len(v) != width or
                    any(type(x) not in (int, float) or not math.isfinite(x) for x in v) for v in values)):
            raise ValueError('invalid mesh vertex channel')
        if name == 'colors' and any(type(x) is not int or not 0 <= x <= 255 for v in values for x in v):
            raise ValueError('invalid mesh vertex colors')


def face_triangles(mesh):
    for f in mesh['faces']:
        yield f[:3]
        if len(f) == 4:
            yield [f[0], f[2], f[3]]


def weights(point, corners):
    a, b, c = corners
    ab, ac, ap = sub(b, a), sub(c, a), sub(point, a)
    normal = cross(ab, ac)
    length = math.sqrt(dot(normal, normal))
    if not length or abs(dot(ap, normal)) > GEOMETRY_EPSILON * length:
        return None
    aa, bb, cc, pa, pc = dot(ab, ab), dot(ab, ac), dot(ac, ac), dot(ap, ab), dot(ap, ac)
    det = aa * cc - bb * bb
    if det <= 0.:
        return None
    u, v = (pa * cc - pc * bb) / det, (pc * aa - pa * bb) / det
    result = [1. - u - v, u, v]
    edge_scale = max(math.dist(a, b), math.dist(a, c), math.dist(b, c))
    if min(result) < -GEOMETRY_EPSILON / edge_scale:
        return None
    result = [max(0., min(1., w)) for w in result]
    total = sum(result)
    return [w / total for w in result]


def interpolate(mesh, face, blend, channel):
    size = CHANNELS[channel][0]
    value = [sum(w * mesh[channel][i][axis] for i, w in zip(face, blend)) for axis in range(size)]
    if channel == 'vertex_normals':
        length = math.sqrt(sum(x * x for x in value))
        if length:
            value = [x / length for x in value]
    return value


def compare(a, b):
    validate(a)
    validate(b)
    present = [name for name in CHANNELS if a[name] is not None]
    if any((a[name] is None) != (b[name] is None) for name in CHANNELS):
        return dict(presence=False, matched=False, maximum=None)
    maximum = dict.fromkeys(CHANNELS, 0.)
    for source, target in [(a, b), (b, a)]:
        for vertex, point in enumerate(source['vertices']):
            candidates = []
            for face in face_triangles(target):
                blend = weights(point, [target['vertices'][i] for i in face])
                if blend is None:
                    continue
                errors = {name: max(abs(x-y) for x, y in zip(source[name][vertex],
                          interpolate(target, face, blend, name))) for name in present}
                score = max((errors[name] / CHANNELS[name][1] for name in errors), default=0.)
                candidates.append((score, errors))
            if not candidates:
                return dict(presence=True, matched=False, maximum=None)
            _, errors = min(candidates, key=lambda item: item[0])
            for name, error in errors.items():
                maximum[name] = max(maximum[name], error)
    return dict(presence=True, matched=all(maximum[k] <= CHANNELS[k][1] for k in CHANNELS), maximum=maximum)
