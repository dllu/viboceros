"""Fresh corner-aperture checks for curve/mesh Near and discrete End/Mid."""
import copy
import argparse
import json
from .point_snaps import request as point_request


def request():
    base = point_request()["operations"][2]
    operations = []
    for view in ("Top","Perspective"):
        for policy in ("line-near","line-end","mesh-near","mesh-mid"):
            for offset in ([-8,-8],[-10,-10],[-11,-11],[-12,-12],[-13,-13],[-10,10],[10,-10],[0,-13]):
                op = copy.deepcopy(base)
                op["id"] = "box-%s-%s-%d-%d" % (view.lower(),policy,offset[0],offset[1])
                op["view"],op["offset"] = view,offset
                op["aim"] = [5.,-2.,7.] if policy == "mesh-mid" else [2.,-2.,7.]
                op["persistent_snaps"] = [dict(near="Near",end="End",mid="Mid")[policy.split("-")[1]]]
                if policy.startswith("line"):
                    op["sources"] = [dict(type="line",start=[2.,-2.,7.],end=[8.,-2.,7.])]
                operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def held_out_request():
    """Slanted wires whose screen-nearest point can lie outside the pick box."""
    base = point_request()["operations"][2]
    operations = []
    for mesh in (False,True):
        for rotation in range(4):
            def rotate(x,y):
                for _ in range(rotation): x,y = -y,x
                return [5.+x,-5.+y,7.]
            for reverse in (False,True):
                for radius in (8,12,16):
                    a,b = rotate(.30,-.45),rotate(.42,.15)
                    other_a,other_b = rotate(10.30,-.45),rotate(10.42,.15)
                    if reverse: a,b = b,a; other_a,other_b = other_b,other_a
                    op = copy.deepcopy(base)
                    op["id"] = "slanted-%d-%d-%d-r%d" % (mesh,rotation,reverse,radius)
                    op["view"],op["aim"],op["offset"],op["capture_radius"] = "Top",[5.,-5.,7.],[0,0],radius
                    op["sources"] = [dict(type="mesh",vertices=[a,b,other_b,other_a],faces=[[0,1,2,3]]) if mesh else
                                     dict(type="line",start=a,end=b)]
                    operations.append(op)
    for original in request()["operations"]:
        if "line-near" not in original["id"]: continue
        op = copy.deepcopy(original)
        op["id"] = op["id"].replace("line-near","line-mid")
        op["persistent_snaps"] = ["Mid"]
        operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def all_request():
    return dict(protocol_version=1,iterations=1,operations=request()["operations"]+held_out_request()["operations"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--held-out",action="store_true")
    group.add_argument("--all",action="store_true")
    args = parser.parse_args()
    print(json.dumps(all_request() if args.all else held_out_request() if args.held_out else request(),indent=2,allow_nan=False))
