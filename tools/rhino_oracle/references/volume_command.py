"""Scalar Volume controls and open-surface diagnostics, from source geometry only."""
import copy
import json
from .volume_centroid import request as closed_request
from .volume_centroid_confirmation import request as warning_request


def request():
    operations = closed_request()["operations"] + warning_request()["operations"]
    for operation in operations:
        operation["op"] = "volume_command"
        operation["collection_api"] = True
    # Pair each surface with both commands. Vary orientation, translation and
    # world axes independently; never obtain sources or targets from captures.
    surface = next(o["sources"][0] for o in operations if o["id"] == "open-surface-pre-yes")
    variants = [("base", surface)]
    for name, transform in [
        ("translated", lambda x,y,z: [x+10,y-20,z+30]),
        ("cycle-axes", lambda x,y,z: [z,x,y]),
        ("swap-xy", lambda x,y,z: [y,x,z]),
        ("scaled", lambda x,y,z: [2*x,3*y,4*z]),
    ]:
        changed = copy.deepcopy(surface)
        for control in changed["control_points"]: control["point"] = transform(*control["point"])
        variants.append((name, changed))
    reversed_patch = copy.deepcopy(surface)
    reversed_patch["control_points"] = [reversed_patch["control_points"][i] for i in (1,0,3,2)]
    variants.append(("reversed-u", reversed_patch))
    for name, source in variants:
        for kind in ("volume_command", "volume_centroid_command"):
            operations.append(dict(op=kind, id="diagnostic-"+name+"-"+kind, sources=[copy.deepcopy(source)],
                                   collection_api=True, open_confirmation="yes"))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2, allow_nan=False))
