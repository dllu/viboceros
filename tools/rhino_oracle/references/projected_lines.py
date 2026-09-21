"""Exact rational reference for clipped straight-line screen proximity.

Independent of both engines: clip against homogeneous W >= near, project the
remaining endpoints, minimize squared screen distance and invert the homography.
All arithmetic before the final binary64 output conversion uses Fraction.
Run as a module to emit the native test's deterministic CSV corpus to stdout.
"""
from fractions import Fraction as F
import itertools


def homogeneous(matrix, point):
    return [sum((F(row[i]) * F(point[i]) for i in range(3)), F(row[3]))
            for row in matrix]


def interpolate(a, b, t):
    return [(1 - t) * F(x) + t * F(y) for x, y in zip(a, b)]


def closest(matrix, a, b, cursor, near):
    """Return exact (model point, squared screen distance), or None if invisible."""
    near = F(near)
    if near <= 0:
        raise ValueError("positive clipping depth required")
    a, b = list(map(F, a)), list(map(F, b))
    ha, hb = homogeneous(matrix, a), homogeneous(matrix, b)
    if ha[2] < near and hb[2] < near:
        return None
    if ha[2] < near or hb[2] < near:
        t = (near - ha[2]) / (hb[2] - ha[2])
        clipped = interpolate(a, b, t)
        if ha[2] < near:
            a, ha = clipped, homogeneous(matrix, clipped)
        else:
            b, hb = clipped, homogeneous(matrix, clipped)
    pa = [ha[i] / ha[2] - F(cursor[i]) for i in range(2)]
    pb = [hb[i] / hb[2] - F(cursor[i]) for i in range(2)]
    delta = [y - x for x, y in zip(pa, pb)]
    squared = sum(x * x for x in delta)
    s = F(0) if squared == 0 else max(F(0), min(F(1), -sum(
        p * d for p, d in zip(pa, delta)) / squared))
    t = s * ha[2] / ((1 - s) * hb[2] + s * ha[2])
    distance_squared = sum(x * x for x in interpolate(pa, pb, s))
    return interpolate(a, b, t), distance_squared


def cases():
    """Finite binary64 inputs, including different camera axes and endpoint orders."""
    for axis, depth, clipped, position, reverse in itertools.product(
            range(3), [1., 32., 1e6, 1e12], [False, True],
            [-1., 0., 0.25, 0.75, 1., 2.], [False, True]):
        # Camera coordinates X,Y,W are a permutation of world coordinates.
        # Exact dyadic coefficients exercise anisotropy, shear and translation.
        matrix = [[0.] * 4 for _ in range(3)]
        matrix[0][axis] = 2.
        matrix[0][(axis + 1) % 3] = 0.5
        matrix[0][(axis + 2) % 3] = 0.25
        matrix[1][(axis + 1) % 3] = -0.5
        matrix[1][(axis + 2) % 3] = -0.75
        matrix[2][(axis + 2) % 3] = 1.
        a, b = [0.] * 3, [0.] * 3
        for j, x in enumerate([-1., 0.25, 1.]):
            a[(axis + j) % 3] = x
        for j, x in enumerate([2., -0.5, -depth if clipped else depth]):
            b[(axis + j) % 3] = x
        if reverse:
            a, b = b, a
        cursor = [position, 0.125]
        near = 0.125
        point, squared = closest(matrix, a, b, cursor, near)
        values = [v for row in matrix for v in row] + a + b + cursor + [near]
        yield values + list(map(float, point)) + [float(squared)]


def csv_text():
    header = "# X,Y,W rows (3x4), a.xyz, b.xyz, cursor.xy, near, target.xyz, distance_squared"
    return header + "\n" + "".join(
        ",".join(format(x, ".17g") for x in row) + "\n" for row in cases())


if __name__ == "__main__":
    print(csv_text(), end="")
