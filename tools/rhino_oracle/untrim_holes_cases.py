"""Source-only component-selection evidence for the pending UntrimHoles command."""
import copy
import json

from .remove_holes_cases import request as geometry_request


def request():
    sources = {op["id"]: op["source"] for op in geometry_request()["operations"]}
    operations = []
    def operation(name, identifier, all_value, component, maximum=0., keep=False,
                  pick="preselect", finish=None):
        result = dict(op="untrim_holes_command", id=identifier,
            sources=[dict(brep=copy.deepcopy(sources[name]))], all=all_value,
            components=[[0, component]], maximum_edge_length=maximum,
            keep_trim_objects=keep, pick=pick)
        if finish is not None: result["finish"] = finish
        return result
    for name, face, edge in (("hole-all", 0, 1), ("two-holes-all", 0, 1),
            ("annulus-all", 0, 1), ("paraboloid-annulus-all", 0, 1),
            ("tube-solid-all", 3, 3), ("tube-open-all", 2, 3),
            ("tube-caps-all", 0, 1), ("outer-all", 0, 0)):
        for all_value in (False, True):
            for keep in (False, True):
                operations.append(operation(name, "%s-all-%d-keep-%d" % (name, all_value, keep),
                    all_value, face if all_value else edge, keep=keep))
    for maximum in (0., 7.9, 8., 8.1):
        for all_value in (False, True):
            operations.append(operation("hole-all", "length-%s-all-%d" % (str(maximum), all_value),
                all_value, 0 if all_value else 1, maximum=maximum))
    for all_value, component in ((False, 1), (True, 0)):
        for finish in ("Enter", "Cancel"):
            operations.append(operation("two-holes-all", "mouse-all-%d-%s" % (all_value, finish),
                all_value, component, keep=True, pick="mouse", finish=finish))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2))
