"""Source-only recipes for Undo within the running native UntrimHoles command."""
import copy
import json

from .remove_holes_cases import request as geometry_request


def request():
    source = next(operation["source"] for operation in geometry_request()["operations"]
                  if operation["id"] == "two-holes-all")
    operations = []
    for all_value in (False, True):
        for finish in ("Enter", "Cancel"):
            for repick in (False, True):
                # Undo the first pick before selecting the second original
                # component. No edited geometry is used as a constructor input.
                components = [[0, 0 if all_value else 1]]
                if repick: components.append([0, 0 if all_value else 2])
                operations.append(dict(op="untrim_holes_command",
                    id="undo-all-%d-%s-repick-%d" % (all_value, finish, repick),
                    sources=[dict(brep=copy.deepcopy(source))], all=all_value,
                    components=components, maximum_edge_length=0.,
                    keep_trim_objects=True, pick="mouse", finish=finish, undo_after=[1]))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2))
