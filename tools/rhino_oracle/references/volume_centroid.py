"""Source-only volume centroid controls, including signed shell aggregation."""
import copy
import json
from pathlib import Path


def tetrahedron(offset=0., scale=1., reverse=False):
    vertices = [[offset, 0., 0.], [offset + 3*scale, 0., 0.],
                [offset, 4*scale, 0.], [offset, 0., 5*scale]]
    faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
    if reverse: faces = [f[::-1] for f in faces]
    return dict(type="mesh", vertices=vertices, faces=faces)


def request():
    operations = []
    def add(name, sources, **options):
        operations.append(dict(op="volume_centroid_command", id=name,
                               sources=copy.deepcopy(sources), **options))
    first, second = tetrahedron(), tetrahedron(10., 2.)
    add("tetrahedron", [first])
    add("reversed-tetrahedron", [tetrahedron(reverse=True)])
    add("translated-tetrahedron", [tetrahedron(1e6)])
    for name, groups, selected in (("separate", [], [0,1]), ("group", [[0,1]], [0,1]),
                                   ("partial-group", [[0,1]], [0]), ("reverse-order", [], [1,0])):
        add(name, [first, second], groups=groups, selected=selected)
    add("postselected", [first, second], preselect=False)
    add("mixed-orientations", [first, tetrahedron(10., 2., True)])
    add("equal-opposite-volumes", [first, tetrahedron(10., 1., True)])
    add("post-equal-opposite-volumes", [first, tetrahedron(10., 1., True)], preselect=False)
    combined = copy.deepcopy(first)
    combined["vertices"].extend(tetrahedron(10., 2., True)["vertices"])
    combined["faces"].extend([[i+4 for i in f] for f in tetrahedron(10., 2., True)["faces"]])
    add("opposed-disjoint-shells", [combined])
    line = dict(type="line", start=[0,0,0], end=[1,0,0])
    add("mixed-line", [first, line])
    add("line-only", [line])
    add("mixed-point", [first, dict(type="point", point=[100,200,300])])
    trimmed = json.loads((Path(__file__).resolve().parents[1]/"fixtures/trimmed_mass_properties.json").read_text())
    for original in trimmed["operations"]:
        if original.get("cap_surface") is None: continue
        source = {k:v for k,v in original.items() if k not in ("op", "id")}
        source["type"] = "brep"
        add("brep-"+original["id"], [source])
    vertices = [[0,0,0],[4,0,0],[4,3,0],[0,3,0],
                [0,0,2],[4,0,2],[4,3,3],[0,3,2]]
    faces = [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]
    for reverse in (False, True):
        for rotation in range(4):
            quad_faces = [f[rotation:]+f[:rotation] for f in faces]
            if reverse: quad_faces = [f[::-1] for f in quad_faces]
            add("warped-box-%d-%d" % (reverse, rotation), [dict(type="mesh", vertices=vertices, faces=quad_faces)])
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2, allow_nan=False))
