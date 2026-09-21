"""Fresh mesh Near competition inputs; no observed targets are read."""
import argparse
import copy
import json
from .point_snaps import held_out_request, request as point_request


def request():
    base = held_out_request()["operations"][4]
    operations = []
    # Same geometry, different raw vertex/face indices and winding.
    for reverse in (False, True):
        for shift in range(4):
            for offset in ([-10,0],[-8,0]):
                op = copy.deepcopy(base)
                op["id"] = "order-%d-%d-%d" % (reverse,shift,offset[0])
                mesh = op["sources"][0]
                order = list(range(4))
                if reverse: order.reverse()
                order = order[shift:]+order[:shift]
                mesh["vertices"] = [mesh["vertices"][i] for i in order]
                mesh["faces"] = [[0,1,2,3]]
                op["offset"] = offset
                operations.append(op)
    # All four corners, with competing wires and changing depth order.
    for corner in range(4):
        for offset in ([0,0],[5,0],[0,5],[-5,-5]):
            op = copy.deepcopy(base)
            op["id"] = "corner-%d-%d-%d" % (corner,offset[0],offset[1])
            op["aim"] = list(op["sources"][0]["vertices"][corner])
            op["offset"] = offset
            operations.append(op)
    # Two parallel interior targets compete without endpoint capture.
    for view in ("Top","Perspective"):
        for offset in range(-4,12,2):
            op = copy.deepcopy(point_request()["operations"][2])
            op["id"] = "parallel-%s-%d" % (view.lower(),offset)
            op["view"],op["aim"],op["offset"] = view,[5.,-2.,7.],[0,offset]
            op["sources"][0]["vertices"] = [[2.,-2.,7.],[8.,-2.,7.],
                                                   [8.,-2.5,7.],[2.,-2.5,7.]]
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def repeat_request():
    """Repeat order witnesses in a changed sequence and with fresh source IDs."""
    originals = request()["operations"][:16]
    operations = []
    for repetition in range(3):
        order = list(range(0,16,2))
        if repetition == 1: order.reverse()
        if repetition == 2: order = order[3:]+order[:3]
        for index in order:
            op = copy.deepcopy(originals[index])
            op["id"] = "repeat-%d-%s" % (repetition,op["id"])
            operations.append(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def picking_request():
    """Public per-wire picking depths to distinguish picking from Near snaps."""
    originals = request()["operations"]
    operations = [copy.deepcopy(originals[i]) for i in list(range(0,16,2))+[38,39,40,44]]
    for op in operations:
        op["id"] = "picking-"+op["id"]
        op["pick_diagnostics"] = True
    return dict(protocol_version=1,iterations=1,operations=operations)


def all_request():
    return dict(protocol_version=1,iterations=1,operations=[
        op for factory in (request,repeat_request,picking_request) for op in factory()["operations"]])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--repeat",action="store_true")
    group.add_argument("--picking",action="store_true")
    group.add_argument("--all",action="store_true")
    args = parser.parse_args()
    print(json.dumps(all_request() if args.all else picking_request() if args.picking else repeat_request() if args.repeat else request(),indent=2,allow_nan=False))
