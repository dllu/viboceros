"""Oriented cyclic contour matching and sampled boundary distances."""
import math

EPSILON = 1e-7


def contour(value):
    if (not isinstance(value, list) or len(value) < 4
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (float, int) or not math.isfinite(x) for x in p) for p in value)
            or math.dist(value[0], value[-1]) > EPSILON):
        raise ValueError('finite closed outline contour required')
    points = value[:-1]
    while len(points) > 3:
        for i, b in enumerate(points):
            a, c = points[i - 1], points[(i + 1) % len(points)]
            ac = [y - x for x, y in zip(a, c)]
            length = sum(x*x for x in ac)
            t = sum((y - x)*z for x, y, z in zip(a, b, ac)) / length if length else -1.
            if 0. <= t <= 1. and math.dist(b, [x + t*d for x, d in zip(a, ac)]) <= EPSILON:
                points = points[:i] + points[i + 1:]
                break
        else:
            break
    return points


def boundary_error(actual, expected, maximum=EPSILON):
    if not isinstance(actual, list) or len(actual) != len(expected):
        raise ValueError('outline contour count differs')
    expected = [contour(p) for p in expected]
    measured = 0.
    for path in actual:
        points = contour(path)
        choices = [(max(math.dist(a, native[(start + i) % len(native)]) for i, a in enumerate(points)), index)
                   for index, native in enumerate(expected) if len(native) == len(points)
                   for start in range(len(native))]
        if not choices or (maximum is not None and min(choices)[0] > maximum):
            raise ValueError('oriented outline boundary differs')
        distance, index = min(choices)
        measured = max(measured, distance)
        expected.pop(index)
    return measured


def sampled_boundary_distance(actual, expected):
    def samples(paths):
        for path in paths:
            yield from path
            for a, b in zip(path, path[1:]):
                yield [(x+y)*.5 for x, y in zip(a, b)]

    def point_segment(p, a, b):
        direction = [y-x for x, y in zip(a, b)]
        squared_length = sum(x*x for x in direction)
        along = sum((y-x)*z for x, y, z in zip(a, p, direction))
        t = max(0., min(1., along / squared_length)) if squared_length else 0.
        return math.dist(p, [x+t*y for x, y in zip(a, direction)])

    def directed(source, target):
        edges = [(p, q) for path in target for p, q in zip(path, path[1:])]
        return max((min(point_segment(p, a, b) for a, b in edges) for p in samples(source)), default=0.)

    if not actual or not expected:
        return 0. if actual == expected else None
    return max(directed(actual, expected), directed(expected, actual))
