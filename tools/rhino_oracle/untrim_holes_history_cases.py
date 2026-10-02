"""Source-only native hole command cases with multiple components and history."""
import copy
import json

from .remove_holes_cases import request as geometry_request


def request():
    base = next(op["source"] for op in geometry_request()["operations"] if op["id"] == "two-holes-all")
    shifted = copy.deepcopy(base)
    for control in shifted["source"]["surface"]["control_points"]:
        control["point"][0] += 20.
    for boundary in shifted["source"]["boundaries"]:
        for control in boundary["curve"]["control_points"]:
            control["point"][0] += 20.
    operations = []
    def add(identifier, components, all_value=False, keep=True, pick="mouse", finish="Enter", undo=None, two=False):
        operation = dict(op="untrim_holes_command", id=identifier,
            sources=[dict(brep=copy.deepcopy(source)) for source in ([base, shifted] if two else [base])],
            all=all_value, components=components, maximum_edge_length=0., keep_trim_objects=keep,
            pick=pick, finish=finish, undo_redo=True)
        if undo: operation["undo_after"] = undo
        operations.append(operation)
    for pick in ("mouse", "preselect"):
        for keep in (False, True):
            for finish in ("Enter", "Cancel"):
                add("both-%s-keep-%d-%s" % (pick, keep, finish), [[0, 1], [0, 2]], keep=keep, pick=pick, finish=finish)
    add("duplicate-edges", [[0, 1], [0, 1]], pick="preselect")
    add("duplicate-faces", [[0, 0], [0, 0]], all_value=True, pick="preselect")
    for finish in ("Enter", "Cancel"):
        add("undo-second-%s" % finish, [[0, 1], [0, 2]], finish=finish, undo=[2])
        for pick in ("mouse", "preselect"):
            add("two-sources-%s-%s" % (pick, finish), [[0, 0], [1, 0]],
                all_value=True, finish=finish, pick=pick, two=True)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2))
