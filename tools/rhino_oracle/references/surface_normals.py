"""Source-only regular surface normal checks; no observed normals are imported."""
import json
import math


def request():
    cases = []
    for size in (1e-6, 1., 1e6):
        for end in (1e-9, 1., 1e9):
            for swapped in (False, True):
                points = [[0., 0., 0.], [size, 0., size], [0., size, 2*size], [size, size, 3*size]]
                if swapped:
                    points = [points[i] for i in (0, 2, 1, 3)]
                cases.append(dict(op="nurbs_surface_evaluate", id="normal-%g-%g-%s" % (size, end, swapped),
                    degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                    control_points=[dict(point=p, weight=1.) for p in points],
                    knots_u=[0., 0., end, end], knots_v=[0., 0., end, end], u=end*0.375, v=end*0.625))
    for sign in (1., -1.):
        cases.append(dict(op="nurbs_surface_evaluate", id="rational-normal-%g" % sign,
            degree_u=2, degree_v=1, control_point_count_u=3, control_point_count_v=2,
            control_points=[dict(point=[x, y, z], weight=sign*w) for z in (0., 3.)
                            for x, y, w in ((1., 0., 1.), (1., 1., math.sqrt(0.5)), (0., 1., 1.))],
            knots_u=[0., 0., 0., 1., 1., 1.], knots_v=[0., 0., 1., 1.], u=0.35, v=0.6))
    return dict(protocol_version=1, iterations=1, operations=cases)


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))
