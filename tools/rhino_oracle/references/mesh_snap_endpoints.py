"""Short-wire endpoint controls, independent of captured target coordinates."""
import argparse
import copy
import json
import itertools
from .point_snaps import request as point_request


def request():
    base = point_request()["operations"][2]
    vertices = [[5.30,-5.45,7.], [5.42,-4.85,7.], [15.42,-4.85,7.], [15.30,-5.45,7.]]
    operations = []
    for view in ("Top","Perspective"):
        for reverse in (False,True):
            for rotation in range(4):
                order = list(range(4))
                if reverse: order.reverse()
                order = order[rotation:]+order[:rotation]
                for radius in (12,16,24,32):
                    op = copy.deepcopy(base)
                    op.update(id="endpoints-%s-%d-%d-r%d" % (view.lower(),reverse,rotation,radius),
                              view=view, aim=[5.,-5.,7.], offset=[0,0], capture_radius=radius,
                              pick_diagnostics=True,
                              sources=[dict(type="mesh",vertices=[vertices[i] for i in order],faces=[[0,1,2,3]])])
                    operations.append(op)
        for reverse in (False,True):
            for radius in (12,16,24,32):
                a,b = vertices[:2]
                if reverse: a,b = b,a
                op = copy.deepcopy(base)
                op.update(id="endpoints-%s-line-%d-r%d" % (view.lower(),reverse,radius),
                          view=view, aim=[5.,-5.,7.], offset=[0,0], capture_radius=radius,
                          sources=[dict(type="line",start=a,end=b)])
                operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def settled_request():
    operations = []
    for delay in (0,250,750):
        for original in request()["operations"]:
            if original["capture_radius"] not in (12,16): continue
            op = copy.deepcopy(original)
            op["id"] = "settle-%d-%s" % (delay,op["id"])
            op["input_settle_ms"] = delay
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def edge_on_request():
    """Valid triangles project all wires onto the short segment or ONE endpoint.

    The other endpoint cannot be explained as an ordinary closest point on an
    adjacent wire. This separates endpoint behavior from competing-wire choice.
    """
    base = point_request()["operations"][2]
    a,b = [5.30,-5.45,7.],[5.42,-4.85,7.]
    operations = []
    for anchor,depth,permutation,radius in itertools.product(range(2),(-10.,1.,10.),range(6),(12,16)):
        c = list((a,b)[anchor]); c[2] += depth
        vertices = [a,b,c]
        order = list(itertools.permutations(range(3)))[permutation]
        op = copy.deepcopy(base)
        op.update(id="edge-on-%d-%d-%d-r%d" % (anchor,depth,permutation,radius),
                  view="Top",aim=[5.,-5.,7.],offset=[0,0],capture_radius=radius,
                  input_settle_ms=250,pick_diagnostics=True,
                  sources=[dict(type="mesh",vertices=[vertices[i] for i in order],faces=[[0,1,2]])])
        operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--settled",action="store_true")
    group.add_argument("--edge-on",action="store_true")
    args = parser.parse_args()
    print(json.dumps(edge_on_request() if args.edge_on else settled_request() if args.settled else request(),indent=2,allow_nan=False))
