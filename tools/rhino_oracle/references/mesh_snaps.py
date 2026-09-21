"""Mesh wire input recipes; no captured engine output is used."""
import argparse
import copy
import json
from .near_snaps import operation
from . import mesh_near, projected_lines, projected_near


def reference_target(item, frame, mesh_weighted=False):
    """Independent mathematical target from sources, camera/click and policy only."""
    if not item["snap_to_meshes"]:
        return None
    pick = item["inputs"][0]["pick"]
    modes = item["persistent_snaps"] if pick["osnap"] == "Persistent" else [pick["osnap"]]
    source = item["sources"][1]
    vertices = source["vertices"]
    wires = sorted(set(tuple(sorted((tuple(vertices[face[i]]), tuple(vertices[face[(i+1)%len(face)]]))))
                       for face in source["faces"] for i in range(len(face))))
    if "Mid" in modes:
        mids = [([(x+y)/2. for x,y in zip(a,b)]) for a,b in wires]
        point = min(mids, key=lambda p: projected_near.distance(frame,p))
        if projected_near.distance(frame,point) <= 12.:
            return dict(kind="Mid", point=point)
    if "Near" in modes:
        matrix = [frame["world_to_screen"][i] for i in (0,1,3)]
        depths = [projected_lines.homogeneous(matrix,p)[2] for p in vertices]
        if min(depths) <= 0:
            raise ValueError("this calibration reference requires fully visible mesh sources")
        candidates = [mesh_near.closest(matrix,a,b,frame["click_client"]) if mesh_weighted else
                      projected_lines.closest(matrix,a,b,frame["click_client"],min(depths)/2) for a,b in wires]
        point, squared = min(candidates, key=lambda pair: pair[1])
        if squared <= 144:
            return dict(kind="Near", point=list(map(float,point)))
    return None


def request():
    quad = dict(type="mesh", vertices=[[2., -2., 7.], [8., -2., 7.],
                                      [8., -8., 7.], [2., -8., 7.]], faces=[[0, 1, 2, 3]])
    triangles = dict(quad, faces=[[0, 1, 2], [0, 2, 3]])
    soup = dict(type="mesh", vertices=quad["vertices"][:3] + [quad["vertices"][0], quad["vertices"][2], quad["vertices"][3]],
                faces=[[0, 1, 2], [3, 4, 5]])
    cases = [
        ("quad-near", quad, [3.5, -2., 7.], "Near", [4, 3]),
        ("quad-mid-hover", quad, [3.5, -2., 7.], "Mid", [0, 0]),
        ("quad-mid-direct", quad, [5., -2., 7.], "Mid", [2, 1]),
        ("triangles-near", triangles, [4., -4., 7.], "Near", [3, 2]),
        ("soup-near", soup, [4., -4., 7.], "Near", [3, 2]),
        ("quad-no-diagonal", quad, [4., -4., 7.], "Near", [0, 0]),
    ]
    operations = []
    for name, source, aim, mode, offset in cases:
        for enabled in [False, True]:
            item = operation(name + ("-enabled" if enabled else "-disabled"), [copy.deepcopy(source)], aim, offset, [mode])
            item["snap_to_meshes"] = enabled
            operations.append(item)
    # One-shot Mid versus mixed-mode direct Mid, with Near as a competing locus.
    for name, aim, modes, one_shot in [
        ("mid-one-shot", [3.5, -2., 7.], ["Point", "Near"], "Mid"),
        ("mid-near-hover", [3.5, -2., 7.], ["Mid", "Near"], "Persistent"),
        ("mid-near-direct", [4.95, -2., 7.], ["Mid", "Near"], "Persistent"),
        ("near-one-shot", [3.5, -2., 7.], ["Point", "Mid"], "Near"),
    ]:
        item = operation(name, [copy.deepcopy(quad)], aim, [0, 0], modes)
        item["inputs"][0]["pick"]["osnap"] = one_shot
        item["snap_to_meshes"] = True
        operations.append(item)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact")
    parser.add_argument("--observations", help="read recorded camera/click only, emit mathematical targets")
    parser.add_argument("--mesh-weighted",action="store_true",help="use measured mesh Near weighting instead of the historical screen-distance reference")
    args = parser.parse_args()
    if args.mesh_weighted and not args.observations: parser.error("mesh weighting requires recorded camera input")
    if args.artifact and args.observations:
        parser.error("artifact requests and camera-derived targets are separate outputs")
    data = request()
    if args.observations:
        with open(args.observations) as source:
            rows = json.load(source)["results"]
        if len(rows) != len(data["operations"]) or any(item["id"] != row["id"] for item,row in zip(data["operations"],rows)):
            raise ValueError("camera observations do not match the mesh request")
        data = {item["id"]: reference_target(item,row["value"]["pick_frames"][0],args.mesh_weighted)
                for item,row in zip(data["operations"],rows)}
    if args.artifact:
        for item in data["operations"]:
            item["sources"][0]["brep"]["artifact_path"] = args.artifact
    print(json.dumps(data, indent=2, allow_nan=False))
