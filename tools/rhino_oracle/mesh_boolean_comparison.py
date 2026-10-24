"""Independent surface samples, triangle distances and polygon mesh measures."""
import math


def sub(a, b):
    return [x-y for x, y in zip(a, b)]


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def triangles(meshes, spatial=True):
    for mesh in meshes:
        vertices = mesh['vertices']
        if (not isinstance(vertices, list) or any(len(p) != 3 or any(
                type(x) not in (int, float) or not math.isfinite(x) for x in p) for p in vertices)):
            raise ValueError('finite mesh vertices required')
        for face in mesh['faces']:
            if len(face) not in (3, 4) or any(type(i) is not int or not 0 <= i < len(vertices) for i in face):
                raise ValueError('valid triangle or quad indices required')
            p = [vertices[i] for i in face]
            if len(p) == 3:
                yield p
            elif spatial and math.dist(p[1], p[3]) < math.dist(p[0], p[2]):
                yield [p[0], p[1], p[3]]
                yield [p[1], p[2], p[3]]
            else:
                yield [p[0], p[1], p[2]]
                yield [p[0], p[2], p[3]]


def point_triangle(p, triangle):
    a, b, c = triangle
    u, v, w = sub(b, a), sub(c, a), sub(p, a)
    n = cross(u, v)
    squared = dot(n, n)
    if squared:
        along_u = dot(cross(w, v), n) / squared
        along_v = dot(cross(u, w), n) / squared
        if along_u >= 0. and along_v >= 0. and along_u+along_v <= 1.:
            return abs(dot(w, n)) / math.sqrt(squared)
    distances = []
    for a, b in [(a, b), (b, c), (c, a)]:
        direction = sub(b, a)
        length = dot(direction, direction)
        t = max(0., min(1., dot(sub(p, a), direction)/length)) if length else 0.
        distances.append(math.dist(p, [x+t*y for x, y in zip(a, direction)]))
    return min(distances)


def sampled_boundary_distance(first, second):
    if not first or not second:
        return 0. if not first and not second else None

    def directed(source, target):
        target = list(triangles(target))
        maximum = 0.
        for points in triangles(source, spatial=False):
            for weights in [(1., 0., 0.), (0., 1., 0.), (0., 0., 1.), (.5, .5, 0.),
                            (0., .5, .5), (.5, 0., .5), (1./3.,)*3]:
                point = [sum(w*p[axis] for w, p in zip(weights, points)) for axis in range(3)]
                maximum = max(maximum, min(point_triangle(point, t) for t in target))
        return maximum
    return max(directed(first, second), directed(second, first))


def measures(meshes):
    area, volume = 0., 0.
    for a, b, c in triangles(meshes):
        n = cross(sub(b, a), sub(c, a))
        area += math.sqrt(dot(n, n))*.5
        volume += dot(a, cross(b, c))/6.
    return area, volume
