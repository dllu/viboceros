"""Exact-rational Bernstein references for separated Bezier elevation fixtures."""
import json
import math
import sys
from fractions import Fraction
from pathlib import Path


def homogeneous(control):
    weight = Fraction(control.get("weight", 1))
    return tuple(Fraction(x) * weight for x in control["point"]) + (weight,)


def coefficients(source_degree, degree, index):
    return [(j, Fraction(math.comb(source_degree, j) * math.comb(degree-source_degree, index-j),
                         math.comb(degree, index)))
            for j in range(max(0, index-(degree-source_degree)), min(source_degree, index)+1)]


def euclidean(point):
    return dict(point=[float(x / point[3]) for x in point[:3]], weight=float(point[3]))


def groups(knots, degree):
    groups = []
    for knot in knots:
        if not groups or groups[-1][0] != knot:
            groups.append([knot, 1])
        else:
            groups[-1][1] += 1
    if any(count != degree+1 for _, count in groups):
        raise ValueError("reference requires separated Bezier blocks")
    return [knot for knot, _ in groups]


def curve_reference(operation):
    source = operation["curve"]
    p, q = source["degree"], operation["degree"]
    sites = groups(source["knots"], p)
    points = [homogeneous(c) for c in source["control_points"]]
    controls = []
    for block in range(len(sites)-1):
        for i in range(q+1):
            controls.append(euclidean(tuple(sum(a * points[block*(p+1)+j][k]
                                           for j, a in coefficients(p, q, i)) for k in range(4))))
    return dict(degree=q, domain=[sites[0], sites[-1]],
                knots=[t for t in sites for _ in range(q+1)], control_points=controls,
                closed=False, periodic=False)


def surface_reference(operation):
    p = [operation["degree_u"], operation["degree_v"]]
    q = [operation["desired_degree_u"], operation["desired_degree_v"]]
    sites = [groups(operation["knots_u"], p[0]), groups(operation["knots_v"], p[1])]
    counts = [(len(s)-1)*(degree+1) for s, degree in zip(sites, q)]
    width = operation["control_point_count_u"]
    points = [homogeneous(c) for c in operation["control_points"]]
    controls = []
    for j in range(counts[1]):
        for i in range(counts[0]):
            controls.append(euclidean(tuple(sum(a*b*points[((j//(q[1]+1))*(p[1]+1)+v)*width
                                                          +(i//(q[0]+1))*(p[0]+1)+u][k]
                                             for u, a in coefficients(p[0], q[0], i%(q[0]+1))
                                             for v, b in coefficients(p[1], q[1], j%(q[1]+1)))
                                             for k in range(4))))
    return dict(degree=q, control_count=counts, domain_u=[sites[0][0], sites[0][-1]],
                domain_v=[sites[1][0], sites[1][-1]], control_points=controls,
                knots_u=[t for t in sites[0] for _ in range(q[0]+1)],
                knots_v=[t for t in sites[1] for _ in range(q[1]+1)], periodic_u=False, periodic_v=False)


def reference(request):
    return {op["id"]: (curve_reference(op) if op["op"] == "curve_change_degree_geometry"
                      else surface_reference(op)) for op in request["operations"]}


if __name__ == "__main__":
    print(json.dumps(reference(json.loads(Path(sys.argv[1]).read_text())), indent=2))
