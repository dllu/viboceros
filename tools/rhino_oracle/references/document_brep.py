"""Source-only document admission matrix, with no expected output baked in."""
import copy
import json


def request():
    def box(center=0, radius=1, reverse=False):
        return dict(source=dict(type="box", min=[center-radius, -radius, -radius],
            max=[center+radius, radius, radius]), reversed=reverse)
    opened = box()
    opened["source"]["keep_faces"] = [0, 1, 2, 3, 4]
    corner = dict(source=dict(type="mesh_brep", vertices=[[0,0,0],[1,10,0],[1,11,1],[2,15,0]],
        faces=[[0,2,1],[0,1,3],[0,3,2],[1,2,3]]), reversed=False)
    recipes = [
        ("box", [box()], []),
        ("negative-volume", [box(-5), box(5, 2, True)], []),
        ("nested-cavity", [box(0, 4), box(0, 1, True)], []),
        ("zero-volume", [box(-3), box(3, 1, True)], []),
        ("coincident", [box(), box(reverse=True)], []),
        ("open", [opened], []),
        ("inconsistent", [box()], [0]),
        ("corner", [corner], []),
    ]
    operations = []
    for name, sources, flip_faces in recipes:
        for reverse in (False, True):
            parts = copy.deepcopy(sources)
            for part in parts: part["reversed"] ^= reverse
            for insertion in ("generic", "no_kink"):
                for selected in (False, True):
                    operations.append(dict(op="document_brep",
                        id="%s-reverse-%s-%s-selected-%s" % (name, reverse, insertion, selected),
                        sources=copy.deepcopy(parts), flip_faces=list(flip_faces),
                        insertion=insertion, selected=selected))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))
