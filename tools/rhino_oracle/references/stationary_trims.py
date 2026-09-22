"""Shared native B-reps with endpoint-copy trim encodings; no expected results."""
import copy
import json


def request():
    shapes = {
        "box": dict(type="box", min=[-1]*3, max=[1]*3),
        "tetrahedron": dict(type="mesh_brep", vertices=[[0,0,0],[1,10,0],[1,11,1],[2,15,0]],
                            faces=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]]),
        "sphere": dict(type="sphere", radius=2),
    }
    encodings = {
        "regular": dict(degree=2, endpoint_indices=[0,1,1], weights=[1,2,1],
                        knots=[-3,-3,-3,7,7,7]),
        "stationary-start": dict(degree=2, endpoint_indices=[0,0,0,1,1], weights=[1,2,1,2,1],
                                 knots=[-3,-3,-3,2,2,7,7,7]),
        "stationary-end": dict(degree=2, endpoint_indices=[0,0,1,1,1], weights=[1,2,1,2,1],
                               knots=[-3,-3,-3,2,2,7,7,7]),
    }
    operations = []
    for kind, source in shapes.items():
        for name, encoding in encodings.items():
            for reversed in (False, True):
                operations.append(dict(op="brep_solid_orientation",
                    id="%s-%s-%s" % (kind, name, "inward" if reversed else "outward"),
                    sources=[dict(source=copy.deepcopy(source), reversed=reversed,
                                  trim_endpoint_encoding=copy.deepcopy(encoding))]))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))
