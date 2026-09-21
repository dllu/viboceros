"""Independent screen-nearest references for the small Near calibration corpus.

No Rhino/native APIs or result coordinates are used. Lines use an exact
projective parameter conversion; circles and single rational quadratic Bezier
spans bracket stationary points of squared screen distance. The fixed bracketing
grid is adequate for these fixtures, not a general certified closest-point API.
All fixture curves are entirely in front of the camera.
"""
import math


def homogeneous(matrix, point):
    return [math.fsum([row[3]] + [row[i] * point[i] for i in range(3)]) for row in matrix]


def project(frame, point):
    h = homogeneous(frame["world_to_screen"], point)
    return [h[i] / h[3] for i in range(2)]


def distance(frame, point):
    pixel = project(frame, point)
    return math.hypot(*(pixel[i] - frame["click_client"][i] for i in range(2)))


def line_point(frame, start, end):
    """Closest point on a fully visible segment, including endpoint clamping."""
    matrix, cursor = frame["world_to_screen"], frame["click_client"]
    a, b = homogeneous(matrix, start), homogeneous(matrix, end)
    if a[3] <= 0 or b[3] <= 0:
        raise ValueError("reference requires positive homogeneous depths")
    pa, pb = [a[i] / a[3] for i in range(2)], [b[i] / b[3] for i in range(2)]
    delta = [pb[i] - pa[i] for i in range(2)]
    squared = math.fsum(v * v for v in delta)
    s = 0. if squared == 0. else max(0., min(1., math.fsum(
        (cursor[i] - pa[i]) * delta[i] for i in range(2)) / squared))
    # Screen fraction s is NOT the model-space fraction in perspective.
    t = s * a[3] / ((1. - s) * b[3] + s * a[3])
    return [(1. - t) * start[i] + t * end[i] for i in range(3)]


def curve_jet(source, t):
    """Analytic position and first derivative in the fixture's [0, 1] domain."""
    if source["type"] == "circle":
        x, n = source["x_axis"], source["normal"]
        y = [n[1]*x[2] - n[2]*x[1], n[2]*x[0] - n[0]*x[2], n[0]*x[1] - n[1]*x[0]]
        angle, radius = math.tau * t, source["radius"]
        c, s = math.cos(angle), math.sin(angle)
        return ([source["center"][i] + radius*(c*x[i] + s*y[i]) for i in range(3)],
                [math.tau*radius*(-s*x[i] + c*y[i]) for i in range(3)])
    if (source["type"] != "nurbs" or source["degree"] != 2 or
            len(source["control_points"]) != 3 or source["knots"] != [0., 0., 0., 1., 1., 1.]):
        raise ValueError("reference only supports circles and one normalized quadratic Bezier span")
    controls = source["control_points"]
    basis, derivative = [(1.-t)**2, 2.*t*(1.-t), t*t], [-2.*(1.-t), 2.-4.*t, 2.*t]
    weight = math.fsum(basis[i]*controls[i]["weight"] for i in range(3))
    dw = math.fsum(derivative[i]*controls[i]["weight"] for i in range(3))
    point = [math.fsum(basis[i]*controls[i]["weight"]*controls[i]["point"][j]
                      for i in range(3))/weight for j in range(3)]
    tangent = [(math.fsum(derivative[i]*controls[i]["weight"]*controls[i]["point"][j]
                         for i in range(3)) - point[j]*dw)/weight for j in range(3)]
    return point, tangent


def stationarity(frame, source, t):
    """Half the derivative of squared distance, without finite differences."""
    point, tangent = curve_jet(source, t)
    matrix, cursor = frame["world_to_screen"], frame["click_client"]
    h = homogeneous(matrix, point)
    dh = [math.fsum(row[i]*tangent[i] for i in range(3)) for row in matrix]
    return math.fsum((h[i]/h[3] - cursor[i]) * (dh[i] - h[i]/h[3]*dh[3])/h[3]
                     for i in range(2))


def near_point(source, frame):
    if source["type"] == "line":
        return line_point(frame, source["start"], source["end"])
    if source["type"] == "polyline":
        points = source["vertices"]
        return min((line_point(frame, a, b) for a, b in zip(points, points[1:])),
                   key=lambda p: distance(frame, p))
    candidates = [0., 1.]
    for index in range(256):
        a, b = index/256., (index+1)/256.
        if stationarity(frame, source, a) <= 0. <= stationarity(frame, source, b):
            for _ in range(64):
                middle = (a+b)/2.
                if middle in (a, b):
                    break
                if stationarity(frame, source, middle) < 0.:
                    a = middle
                else:
                    b = middle
            candidates.append((a+b)/2.)
    return min((curve_jet(source, t)[0] for t in candidates), key=lambda p: distance(frame, p))
