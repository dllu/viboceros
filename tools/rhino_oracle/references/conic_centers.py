"""Independent conic definitions for API and calibrated point-prompt probes.

Emit JSON to stdout; this generator does not launch either engine or read their
outputs. Homogeneous Bernstein elevation/subdivision preserves the input locus
up to binary64 rounding, which is deliberately retained in the fixtures.
"""
import argparse
import copy
import json
import math


def cp(point, weight=1.):
    return dict(point=list(point), weight=weight)


def quarter():
    return dict(degree=2, control_points=[cp([2., -4., 0.]), cp([2., -3., 0.], math.sqrt(.5)),
                                        cp([4., -3., 0.])], knots=[0., 0., 0., 1., 1., 1.])


def ellipse():
    points = [[2., -4., 0.], [2., -3., 0.], [4., -3., 0.], [6., -3., 0.],
              [6., -4., 0.], [6., -5., 0.], [4., -5., 0.], [2., -5., 0.], [2., -4., 0.]]
    return dict(degree=2, control_points=[cp(p, math.sqrt(.5) if i % 2 else 1.) for i, p in enumerate(points)],
                knots=[0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.])


def blend(a, b, t):
    weight = math.fsum([a["weight"]*(1-t), b["weight"]*t])
    return cp([math.fsum([a["point"][i]*a["weight"]*(1-t), b["point"][i]*b["weight"]*t])/weight
               for i in range(3)], weight)


def elevated(curve, degree):
    curve = copy.deepcopy(curve)
    while curve["degree"] < degree:
        p, n = curve["control_points"], curve["degree"]+1
        curve = dict(degree=n, control_points=[p[0]] + [blend(p[i], p[i-1], i/n) for i in range(1,n)] + [p[-1]],
                     knots=[0.]*(n+1)+[1.]*(n+1))
    return curve


def split_quadratic(curve, t, keep_both):
    p = curve["control_points"]
    a, b = blend(p[0], p[1], t), blend(p[1], p[2], t)
    middle = blend(a, b, t)
    return dict(degree=2, control_points=[p[0], a, middle, b, p[2]] if keep_both else [p[0], a, middle],
                knots=[0., 0., 0., t, t, 1., 1., 1.] if keep_both else [0., 0., 0., 1., 1., 1.])


def case(name, curve=None, aim=None, center=None, ui=True):
    return dict(id=name, curve=curve or quarter(), aim=aim or [2.4, -3.4, 0.],
                center=center or [4., -4., 0.], ui=ui)


def mapped(source, transform, name):
    result = copy.deepcopy(source)
    result["id"] = name
    for control in result["curve"]["control_points"]:
        control["point"] = transform(control["point"])
    for field in ["aim", "center"]:
        result[field] = transform(result[field])
    return result


def cases():
    out = [case("ellipse", ellipse()), case("quarter")]
    reverse = quarter()
    reverse["control_points"].reverse()
    out.append(case("reversed", reverse))
    for degree in [5, 12]:
        out.append(case("elevated-%d" % degree, elevated(quarter(), degree)))
    for fraction in [.5, 1e-6]:
        out.append(case("refined-%g" % fraction, split_quadratic(quarter(), fraction, True)))
    for name, a, b in [("shifted", 1e12, 2.), ("tiny-domain", 0., 1e-170), ("large-domain", 0., 1e170)]:
        curve = quarter()
        curve["knots"] = [a+b*t for t in curve["knots"]]
        out.append(case(name, curve))
    for name, gauge in [("positive-gauge", 8.), ("negative-gauge", -8.), ("tiny-gauge", 1e-200), ("large-gauge", 1e200)]:
        curve = ellipse()
        for control in curve["control_points"]:
            control["weight"] *= gauge
        out.append(case(name, curve))
    curve = quarter()
    for i, control in enumerate(curve["control_points"]):
        control["weight"] *= .25**i
    out.append(case("projective-weights", curve))
    p = quarter()["control_points"]
    stationary = dict(degree=4, control_points=[p[0], p[0], blend(p[0], p[1], 1/3), p[1], p[2]],
                      knots=[0.]*5+[1.]*5)
    out.append(case("stationary-quartic", stationary))
    out.append(mapped(case("base"), lambda p: [4+.8*(p[0]-4)-.6*(p[1]+4), -4+.6*(p[0]-4)+.8*(p[1]+4), p[2]], "rotated"))
    out.append(mapped(case("base"), lambda p: [p[0]+.5*(p[1]+4), p[1], p[2]], "sheared"))
    out.append(mapped(case("base"), lambda p: [p[0], p[1], p[2]+7], "off-plane"))
    out.append(mapped(case("base"), lambda p: [4+.6*(p[0]-4)-4/math.sqrt(50)*(p[1]+4),
                                              -4+.8*(p[0]-4)+3/math.sqrt(50)*(p[1]+4),
                                              7+5/math.sqrt(50)*(p[1]+4)], "tilted"))
    far = mapped(case("base", ellipse(), ui=False), lambda p: [p[0]+1e12, p[1]-1e12, p[2]+1e12], "far-origin")
    out.append(far)
    for factor in [.1, .01]:
        out.append(mapped(case("base", ellipse()), lambda p, f=factor: [p[0], -4+f*(p[1]+4), p[2]], "eccentric-%g" % factor))
    for fraction in [.1, .01, .001]:
        curve = split_quadratic(quarter(), fraction, False)
        p = curve["control_points"]
        aim = blend(blend(p[0], p[1], .5), blend(p[1], p[2], .5), .5)["point"]
        out.append(case("short-arc-%g" % fraction, curve, aim))
    for name, weight in [("parabola", 1.), ("hyperbola", 2.)]:
        curve = quarter()
        curve["control_points"][1]["weight"] = weight
        out.append(case(name, curve))
    for name, axis, amount in [("altered-span", 0, .2), ("nonplanar", 2, .2), ("near-conic", 0, 1e-5)]:
        curve = ellipse()
        curve["control_points"][7]["point"][axis] += amount
        out.append(case(name, curve))
    out.append(mapped(case("base", ellipse()), lambda p: [p[0], -4+2*(p[1]+4), p[2]], "circle"))
    return out


def request(mode):
    operations = []
    for item in cases():
        if mode == "api":
            operations.append(dict(op="nurbs_curve_conic_centers", id=item["id"], curve=item["curve"]))
        elif item["ui"]:
            for persistent in [False, True]:
                operations.append(dict(op="split_edge_command", id=item["id"]+("-persistent" if persistent else "-one-shot"),
                    sources=[dict(brep=dict(source=dict(type="box", min=[0.,0.,0.], max=[10.,12.,14.]))),
                             dict(item["curve"], type="nurbs")], selected=[0], edge=0, pick="mouse",
                    inputs=[dict(pick=dict(point=item["center"], aim=item["aim"], offset=[0,0],
                                           osnap="Persistent" if persistent else "Cen"))],
                    persistent_snaps=["Cen"] if persistent else ["Point", "End", "Mid", "Cen", "Quad"],
                    record_viewport=True, undo_redo=True, trace_commands=True))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["api", "snaps"])
    print(json.dumps(request(parser.parse_args().mode), indent=2, allow_nan=False))
