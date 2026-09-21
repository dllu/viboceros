"""Competing Near targets as separate curves, separate meshes, or one mesh."""
import copy
import json
from .point_snaps import request as point_request


def request():
    base = point_request()["operations"][2]
    operations = []
    for view in ("Top","Perspective"):
        for representation in ("lines","meshes","combined"):
            for reverse in (False,True):
                for offset in (-4,0,4,8,10,14):
                    op = copy.deepcopy(base)
                    op["id"] = "sources-%s-%s-%d-%d" % (view.lower(),representation,reverse,offset)
                    op["view"],op["aim"],op["offset"] = view,[5.,-2.,7.],[0,offset]
                    lines = [dict(type="line",start=[2.,y,7.],end=[8.,y,7.]) for y in (-2.,-2.5)]
                    meshes = [dict(type="mesh",vertices=[line["start"],line["end"],
                              [8.,-10.+line["start"][1],7.],[2.,-10.+line["start"][1],7.]],faces=[[0,1,2,3]]) for line in lines]
                    if reverse:
                        lines.reverse(); meshes.reverse()
                    if representation == "combined":
                        op["sources"] = [dict(type="mesh",vertices=meshes[0]["vertices"]+meshes[1]["vertices"],
                                              faces=[[0,1,2,3],[4,5,6,7]])]
                    else:
                        op["sources"] = lines if representation == "lines" else meshes
                    operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__ == "__main__": print(json.dumps(request(),indent=2,allow_nan=False))
