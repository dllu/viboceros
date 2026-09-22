"""Unconstrained point-snap inputs, independently defined without engine output."""
import copy
import json
import argparse
from . import mesh_near, projected_lines, projected_near


def reference_target(operation, frame):
    """Source/camera/policy-only target; does not use the observed picked point."""
    source = operation["sources"][0]
    mesh = source["type"] == "mesh"
    if mesh and not operation["snap_to_meshes"]: return None
    if mesh:
        edges = mesh_near.wires(source)
    else: edges = [(source["start"],source["end"])]
    radius = operation.get("capture_radius",12)
    modes = operation["persistent_snaps"]
    matrix = [frame["world_to_screen"][i] for i in (0,1,3)]
    if "End" in modes and not mesh:
        admitted = [p for p in edges[0] if projected_near.in_square(frame,p,radius)]
        if admitted: return dict(kind="End",point=min(admitted,key=lambda p:projected_near.distance(frame,p)))
    if "Mid" in modes:
        mids = [[(a+b)/2 for a,b in zip(*edge)] for edge in edges]
        admitted = [p for p in mids if projected_near.in_square(frame,p,radius)]
        if not mesh and modes == ["Mid"]:
            depths = [projected_lines.homogeneous(matrix,p)[2] for p in edges[0]]
            hover,_ = projected_lines.closest(matrix,*edges[0],frame["click_client"],min(depths)/2)
            if projected_near.in_square(frame,hover,radius): admitted = mids
        if admitted: return dict(kind="Midpoint",point=min(admitted,key=lambda p:projected_near.distance(frame,p)))
    if "Near" in modes:
        if mesh:
            candidates = [mesh_near.closest(matrix,*edge,frame["click_client"],radius) for edge in edges]
        else:
            depths = [projected_lines.homogeneous(matrix,p)[2] for p in edges[0]]
            candidates = [projected_lines.closest(matrix,*edges[0],frame["click_client"],min(depths)/2)]
        candidates = [row for row in candidates if projected_near.in_square(frame,row[0],radius)]
        if candidates:
            target,_ = min(candidates,key=lambda row:row[1])
            return dict(kind="Near",point=list(map(float,target)))
    return None


def request():
    quad = dict(type="mesh", vertices=[[2.,-2.,7.],[8.,-2.,7.],[8.,-8.,7.],[2.,-8.,7.]], faces=[[0,1,2,3]])
    triangles = dict(quad, faces=[[0,1,2],[0,2,3]])
    line = dict(type="line", start=quad["vertices"][0], end=quad["vertices"][1])
    cases = []
    for name, source, aim in (("line",line,[3.5,-2.,7.]), ("quad",quad,[3.5,-2.,7.]), ("triangles",triangles,[4.,-4.,7.])):
        for label, offset in (("on",[0,0]),("offset",[4,3])):
            cases.append((name+"-near-"+label,source,aim,offset,["Near"],True))
    cases += [
        ("quad-mid-hover",quad,[3.5,-2.,7.],[0,0],["Mid"],True),
        ("quad-mid-direct",quad,[5.,-2.,7.],[2,1],["Mid"],True),
        ("line-mid-hover",line,[3.5,-2.,7.],[0,0],["Mid"],True),
        ("quad-disabled",quad,[3.5,-2.,7.],[0,0],["Near"],False),
        ("quad-no-modes",quad,[3.5,-2.,7.],[0,0],[],True),
        ("quad-no-diagonal",quad,[4.,-4.,7.],[0,0],["Near"],True),
    ]
    operations = []
    for view in ("Perspective", "Top"):
        for name, source, aim, offset, modes, enabled in cases:
            operations.append(dict(op="point_snap", id=view.lower()+"-"+name,
                                   sources=[copy.deepcopy(source)], view=view,
                                   bounds=[[0.,-10.,0.],[10.,0.,10.]], aim=aim, offset=offset,
                                   persistent_snaps=modes, snap_to_meshes=enabled))
    return dict(protocol_version=1, iterations=1, operations=operations)


def held_out_request():
    """Fresh witnesses for endpoint-depth weighting; no discovery outputs read."""
    base = request()["operations"][2]  # Perspective quad Near, on its top edge.
    transforms = [
        ("reversed",lambda p:list(p),True),
        ("tilted",lambda p:[p[0]+p[2]/4,p[1]+p[0]/2,p[2]+p[1]/2],False),
        ("small",lambda p:[p[0]/64,p[1]/64,p[2]/64],False),
        ("large",lambda p:[64*p[0]+1024,64*p[1]-2048,64*p[2]+512],False),
        ("side",lambda p:[p[2],p[0],p[1]],False),
        ("front",lambda p:[p[0],p[2],-p[1]],False),
    ]
    operations = []
    for name, transform, reverse in transforms:
        for offset in ([0,0],[5,-4],[-4,5],[9,7]):
            op = copy.deepcopy(base)
            op["id"] = "%s-%d-%d" % (name,offset[0],offset[1])
            source = op["sources"][0]
            source["vertices"] = [transform(p) for p in source["vertices"]]
            if reverse:
                source["vertices"].reverse()
                source["faces"] = [[3-i for i in face] for face in source["faces"]]
            op["aim"] = transform(op["aim"])
            corners = [transform([x,y,z]) for x in (0.,10.) for y in (-10.,0.) for z in (0.,10.)]
            op["bounds"] = [[fn(p[i] for p in corners) for i in range(3)] for fn in (min,max)]
            op["offset"] = offset
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def threshold_request():
    """Near-cursor controls for the held-out side-edge discrepancy."""
    witnesses = held_out_request()["operations"]
    cases = [(witnesses[4],[[x,0] for x in range(-10,11)]),
             (witnesses[16],[[10,-7],[4,-2],[-9,9],[-2,3],[-3,4],[-8,8],
                             [3,-1],[9,-6],[5,-3],[-10,10],[-1,2],[-4,5],
                             [-7,7],[2,0],[8,-5],[6,-4],[0,1],[-5,6],[-6,6],[1,1]])]
    operations = []
    for base, offsets in cases:
        for offset in offsets:
            op = copy.deepcopy(base)
            op["id"] = "threshold-%s-%d-%d" % (base["id"].split("-")[0],offset[0],offset[1])
            op["offset"] = offset
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def aperture_request():
    """Fresh check of the endpoint-in-square branch at multiple apertures."""
    witnesses = held_out_request()["operations"]
    operations = []
    for base,offset in ((witnesses[4],[-8,0]),(witnesses[16],[-2,3]),(witnesses[16],[-4,5])):
        for radius in (8,10,12,16):
            op = copy.deepcopy(base)
            op["id"] = "aperture-%s-%d-%d-r%d" % (base["id"].split("-")[0],offset[0],offset[1],radius)
            op["offset"], op["capture_radius"] = offset,radius
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def all_request():
    return dict(protocol_version=1,iterations=1,operations=[
        op for factory in (request,held_out_request,threshold_request,aperture_request)
        for op in factory()["operations"]])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--held-out",action="store_true")
    group.add_argument("--threshold",action="store_true")
    group.add_argument("--aperture",action="store_true")
    group.add_argument("--all",action="store_true")
    parser.add_argument("--observations",help="emit independent targets from recorded cameras, never picked points")
    args = parser.parse_args()
    data = all_request() if args.all else aperture_request() if args.aperture else threshold_request() if args.threshold else held_out_request() if args.held_out else request()
    if args.observations:
        with open(args.observations) as stream: rows = json.load(stream)["results"]
        if [op["id"] for op in data["operations"]] != [row["id"] for row in rows]:
            raise ValueError("camera observations do not match input requests")
        data = {op["id"]:reference_target(op,row["value"]["frame"]) for op,row in zip(data["operations"],rows)}
    print(json.dumps(data, indent=2, allow_nan=False))
