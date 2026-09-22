"""Shared-source classifier probes, including deliberately unresolved inputs."""
import copy
import json

from .orientation_audit import compound_request, spatial_request


def request():
    cases = []
    for data in (compound_request(), spatial_request()):
        for operation in data["operations"]:
            sources = []
            for part in operation["sources"][0]["parts"]:
                center = part.get("translation", [part.get("offset", 0), 0, 0])
                size = part.get("size", 1)
                sources.append(dict(source=dict(type="box", min=[c-size for c in center],
                                                max=[c+size for c in center]), reversed=part.get("reversed", False)))
            cases.append(dict(op="brep_solid_orientation", id=operation["id"], sources=sources))
    box = dict(source=dict(type="box", min=[-1]*3, max=[1]*3))
    opened = copy.deepcopy(box)
    opened["source"]["keep_faces"] = [0, 1, 2, 3, 4]
    cases.append(dict(op="brep_solid_orientation", id="open-box", sources=[opened]))
    for faces in ([0], [0, 1, 2]):
        cases.append(dict(op="brep_solid_orientation", id="unoriented-%d" % len(faces),
                          sources=[copy.deepcopy(box)], flip_faces=faces))
    tetrahedron = dict(type="mesh_brep", vertices=[[0,0,0],[1,10,0],[1,11,1],[2,15,0]],
                       faces=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]])
    for reverse in (False, True):
        cases.append(dict(op="brep_solid_orientation", id="corner-%s" % reverse,
                          sources=[dict(source=copy.deepcopy(tetrahedron), reversed=reverse)]))
    return dict(protocol_version=1, iterations=1, operations=cases)


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))
