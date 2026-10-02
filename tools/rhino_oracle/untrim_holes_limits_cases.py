"""Independent polygon-hole recipes for the native maximum edge length filter."""
import copy
import json

from .remove_holes_cases import request as geometry_request


def request():
    base = next(operation["source"] for operation in geometry_request()["operations"]
                if operation["id"] == "hole-all")
    operations = []
    for name, maxima in (("square", (0., 1.999, 2., 2.001, 7.999, 8., 8.001)),
                        ("unequal", (0., 1., 3.999, 4., 9.999, 10., 10.001))):
        source = copy.deepcopy(base)
        source["splits"] = [[1, [1., 2., 3.]]]
        if name == "unequal":
            points = [[3., 3., 0.], [3., 4., 0.], [7., 4., 0.], [7., 3., 0.], [3., 3., 0.]]
            for kind in ("curve", "parameter_curve"):
                source["source"]["boundaries"][1][kind]["control_points"] = [
                    dict(point=point, weight=1.) for point in points]
        for maximum in maxima:
            # SplitEdge keeps the first span at edge 1 and appends the others
            # from last to first. Edge 1 is short, edge 2 is long in the rectangle.
            for all_value, component in ((True, 0), (False, 1), (False, 2)):
                operations.append(dict(op="untrim_holes_command",
                    id="%s-length-%s-all-%d-c-%d" % (name, str(maximum), all_value, component),
                    sources=[dict(brep=copy.deepcopy(source))], all=all_value,
                    components=[[0, component]], maximum_edge_length=maximum,
                    keep_trim_objects=True, pick="preselect"))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2))
