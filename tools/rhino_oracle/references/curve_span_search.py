"""Closest-point search coverage inputs, independent of either engine's answers."""
import copy
import json
import math


def request():
    control = lambda point, weight=1.: dict(point=point, weight=weight)
    w = math.sqrt(.5)
    s = .99
    a, b, c = (1-s)**2, 2*w*s*(1-s), s*s
    xy = [(a+b)/(a+b+c), (b+c)/(a+b+c)]
    shapes = {
        "stationary-line": (dict(degree=2, control_points=[
            control([x,0.,0.],weight) for x,weight in zip([0.,0.,1.,1.,1.],[1.,2.,1.,2.,1.])],
            knots=[-3.,-3.,-3.,2.,2.,7.,7.,7.]), [.99,0.]),
        "stationary-arc": (dict(degree=2, control_points=[
            control([x,y,0.],weight) for x,y,weight in
            [(1.,0.,1.),(1.,1.,w),(0.,1.,1.),(0.,1.,2.),(0.,1.,1.)]],
            knots=[0.,0.,0.,.5,.5,1.,1.,1.]), xy),
        "decoy-polyline": (dict(degree=1, control_points=[
            control([0.,0.,0.]),control([1000.,0.,0.])]+
            [control([990.,i/8.,0.]) for i in range(1,33)],
            knots=[0.,0.]+[i/33. for i in range(1,33)]+[1.,1.]), [990.,0.]),
    }
    operations = []
    for kind, (definition, xy) in shapes.items():
        for reverse in (False, True):
            for label, z in (("on",0.),("offset",3.),("distant",1e100)):
                curve = copy.deepcopy(definition)
                if reverse:
                    curve["control_points"].reverse()
                    curve["knots"] = [-k for k in reversed(curve["knots"])]
                operations.append(dict(op="nurbs_curve_closest_point",
                    id="%s-%s-%s" % (kind,"reverse" if reverse else "forward",label),
                    target=xy+[z],**curve))
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-10),operations=operations)


if __name__ == "__main__":
    print(json.dumps(request(),indent=2,allow_nan=False))
