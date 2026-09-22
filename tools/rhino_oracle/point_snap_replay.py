"""Replay saved GetPoint calibration natively; mismatches remain failures.

Only source geometry, camera/click and snap policy enter the native operation.
None/unsnapped results compare admission, not arbitrary CPlane placement.
"""
import argparse
import copy
import json
import math
from . import point_snap_probe as probe
from .client import OracleClient, OracleError, OracleProtocolError, _validate_epsilon, _validate_response, compare_responses, load_request


def camera_input(operation, frame):
    def vector(value, size):
        return isinstance(value,list) and len(value) == size and all(probe.finite(x) for x in value)
    if not isinstance(frame,dict): raise OracleProtocolError("missing calibrated camera")
    matrix = frame.get("world_to_screen")
    location,direction,cursor = [frame.get(key) for key in ("camera_location","camera_direction","click_client")]
    size = frame.get("size")
    if (not isinstance(matrix,list) or len(matrix) != 4 or not all(vector(row,4) for row in matrix)
            or not any(matrix[3]) or not vector(location,3) or not vector(direction,3) or not any(direction)
            or not vector(cursor,2) or not isinstance(size,list) or len(size) != 2
            or any(type(v) is not int or v < 3 for v in size)
            or any(type(v) is not int or not 1 <= v < limit-1 for v,limit in zip(cursor,size))
            or frame.get("aim") != operation["aim"] or not vector(frame.get("aim_client"),2)):
        raise OracleProtocolError("invalid camera/click calibration")
    if cursor != [int(v)+offset for v,offset in zip(frame["aim_client"],operation["offset"])]:
        raise OracleProtocolError("calibrated click does not match requested offset")
    h = [sum(a*b for a,b in zip(row,operation["aim"]+[1.])) for row in matrix]
    if (any(not math.isfinite(v) for v in h) or h[3] == 0
            or any(abs(h[i]/h[3]-frame["aim_client"][i]) > 1e-7 for i in range(2))):
        raise OracleProtocolError("calibrated matrix does not match viewport aim")
    return dict(world_to_screen=copy.deepcopy(matrix),location=list(location),direction=list(direction))


def prepare(request, observed):
    """Validate owned observations; return native inputs and target-only evidence."""
    probe.validate_request(request)
    _validate_response(observed,"rhino")
    if observed["iterations"] != 1 or [row["id"] for row in observed["results"]] != [op["id"] for op in request["operations"]]:
        raise OracleProtocolError("calibrated observations do not match requested operations")
    operations, targets = [], []
    for operation,row in zip(request["operations"],observed["results"]):
        value = row["value"]
        if not isinstance(value,dict): raise OracleProtocolError("missing point observation")
        delay = operation.get("input_settle_ms",0)
        motion = value.get("input_motion")
        if delay:
            if (not isinstance(motion,dict) or set(motion) != {"requested_settle_ms","motion_to_click_ms","detour_pixels"}
                    or type(motion["requested_settle_ms"]) is not int or motion["requested_settle_ms"] != delay
                    or type(motion["detour_pixels"]) is not int or motion["detour_pixels"] != 1
                    or not probe.finite(motion["motion_to_click_ms"]) or motion["motion_to_click_ms"] < delay):
                raise OracleProtocolError("unverified owned point input settling")
        elif motion is not None:
            raise OracleProtocolError("unexpected owned point input settling")
        frame = value.get("frame")
        camera = camera_input(operation,frame)
        kind,source,point = [value.get(key) for key in ("kind","source","point")]
        if not probe.point(point): raise OracleProtocolError("invalid observed point")
        if kind == "None":
            if source is not None: raise OracleProtocolError("unsnapped point has an object source")
            point = None
        elif kind not in ("Point","End","Midpoint","Center","Quadrant","Near") or type(source) is not int or not 0 <= source < len(operation["sources"]):
            raise OracleProtocolError("invalid observed snap kind/source")
        expected = [dict(line=[s["start"],s["end"]]) if s["type"] == "line" else
                    dict(mesh=dict(vertices=s["vertices"],faces=s["faces"])) for s in operation["sources"]]
        if value.get("before") != expected or value["before"] != value.get("after"):
            raise OracleProtocolError("source geometry differs from input or changed during snap capture")
        state = value.get("mesh_snap_setting")
        if (not isinstance(state,dict) or type(state.get("before")) is not bool or type(state.get("restored")) is not bool
                or state["before"] != state.get("restored") or state.get("requested") is not operation["snap_to_meshes"]):
            raise OracleProtocolError("mesh snap setting was not verified/restored")
        operations.append(dict(op="projected_object_snap",id=operation["id"],sources=copy.deepcopy(operation["sources"]),
                               camera=camera,cursor=list(frame["click_client"]),capture_radius=operation.get("capture_radius",12),
                               modes=list(operation["persistent_snaps"]),snap_to_meshes=operation["snap_to_meshes"]))
        targets.append(dict(id=row["id"],value=dict(kind=kind,source=source,point=copy.deepcopy(point)),elapsed_ns=0))
    native = dict(protocol_version=1,iterations=1,operations=operations)
    if "tolerance" in request: native["tolerance"] = copy.deepcopy(request["tolerance"])
    evidence = dict(protocol_version=1,iterations=1,engine="rhino",engine_version=observed.get("engine_version"),results=targets,error=None)
    return native,evidence


def replay(request, observed, client=None, absolute_epsilon=1e-9):
    _validate_epsilon(absolute_epsilon,"absolute")
    native,evidence = prepare(request,observed)
    actual = (client or OracleClient()).run_viboceros(native)
    return compare_responses(actual,evidence,absolute_epsilon=absolute_epsilon,relative_epsilon=0.)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request")
    parser.add_argument("observations")
    parser.add_argument("--emit-native",action="store_true",help="emit calibrated inputs without running either engine")
    args = parser.parse_args()
    try:
        request,observed = load_request(args.request),load_request(args.observations)
        if args.emit_native:
            output = prepare(request,observed)[0]
            passed = True
        else:
            report = replay(request,observed)
            output = report.as_dict()
            output["scope"] = "full 3D snap target/kind/source; None compares admission only, not CPlane placement; no history or speed claim"
            passed = report.passed
        print(json.dumps(output,indent=2,allow_nan=False))
        return 0 if passed else 1
    except (ValueError,OSError,OracleError) as error:
        parser.exit(2,"point snap replay failed: %s\n" % error)


if __name__ == "__main__":
    raise SystemExit(main())
