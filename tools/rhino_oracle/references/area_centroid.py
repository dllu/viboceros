"""Source-only AreaCentroid grouping and geometric controls."""
import copy
import json
import argparse
from pathlib import Path


def initial_request():
    circles = [dict(type="circle", center=[x,0,0], radius=r, normal=[0,0,1],x_axis=[1,0,0]) for x,r in ((0,1),(10,2),(0,3))]
    operations = []
    for name, groups, selected in (("separate",[],[0,1,2]),("group",[[0,1,2]],[0,1,2]),
                                    ("partial-group",[[0,1,2]],[0,1]),("nested",[[0,1],[0,1,2]],[0,1,2]),
                                    ("overlap",[[0,1],[1,2]],[0,1,2]),("reverse",[],[2,1,0])):
        operations.append(dict(op="area_centroid_command",id=name,sources=copy.deepcopy(circles),groups=groups,selected=selected))
    polygon = [[0,0,0],[6,0,0],[6,1,0],[1,1,0],[1,4,0],[0,4,0],[0,0,0]]
    shapes = [dict(type="polyline",vertices=polygon),
              dict(type="nurbs",degree=3,control_points=[[0,0,0],[1,0,0],[0,1,0],[0,0,0]],knots=[0,0,0,0,1,1,1,1]),
              dict(type="mesh",vertices=[[0,0,0],[6,0,0],[0,3,0],[20,0,0],[22,0,0],[20,2,0]],faces=[[0,1,2],[3,4,5]]),
              dict(type="mesh",vertices=[[0,0,0],[4,0,0],[4,3,2],[0,3,0]],faces=[[0,1,2,3]]),
              dict(type="surface",degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,
                   control_points=[[0,0,0],[4,0,0],[0,3,0],[4,3,2]],knots_u=[0,0,1,1],knots_v=[0,0,1,1])]
    for i, shape in enumerate(shapes):
        if "control_points" in shape:
            shape["control_points"] = [dict(point=p,weight=1.) for p in shape["control_points"]]
        operations.append(dict(op="area_centroid_command",id="shape-%d" % i,sources=[shape]))
    operations.append(dict(op="area_centroid_command",id="mixed-group",sources=[circles[0],shapes[2]],groups=[[0,1]]))
    return dict(protocol_version=1,iterations=1,operations=operations)


def quad_request():
    operations=[]
    quads=[[[0,0,0],[4,0,0],[4,3,2],[0,3,0]],
           [[-2,0,0],[0,-1,0],[2,0,0],[0,2,3**0.5]],
           [[0,0,0],[4,0,0],[1,1,0],[0,3,0]],
           [[-2,0,0],[0,-1,0],[3,0,0],[0,2,4]]]
    for shape,vertices in enumerate(quads):
        for reverse in (False,True):
            for rotation in range(4):
                order=list(range(4))
                if reverse: order.reverse()
                order=order[rotation:]+order[:rotation]
                operations.append(dict(op="area_centroid_command",id="quad-%d-%d-%d"%(shape,reverse,rotation),
                    sources=[dict(type="mesh",vertices=[vertices[i] for i in order],faces=[[0,1,2,3]])]))
    return dict(protocol_version=1,iterations=1,operations=operations)


def request():
    data=initial_request()
    for index in (0,1):
        op=copy.deepcopy(data["operations"][index]); op.update(id="post-"+op["id"],preselect=False)
        data["operations"].append(op)
    line=dict(type="line",start=[0,0,0],end=[1,0,0])
    data["operations"].append(dict(op="area_centroid_command",id="mixed-open",sources=[data["operations"][0]["sources"][0],line]))
    data["operations"].append(dict(op="area_centroid_command",id="open-only",sources=[line]))
    trimmed=json.loads((Path(__file__).resolve().parents[1]/"fixtures/trimmed_mass_properties.json").read_text())
    for original in trimmed["operations"]:
        source={k:v for k,v in original.items() if k not in ("op","id")}
        source["type"]="brep"
        data["operations"].append(dict(op="area_centroid_command",id="brep-"+original["id"],sources=[source]))
    data["operations"].extend(quad_request()["operations"])
    return data


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--quads",action="store_true")
    args=parser.parse_args()
    print(json.dumps(quad_request() if args.quads else request(),indent=2,allow_nan=False))
