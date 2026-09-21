"""Independent Near snap inputs; no engine outputs or inferred targets are read.

The declared point is an aim hypothesis, not the recorded snap result. Integer
pixel rounding and offsets move the actual projected target; native replay must
compute it from the recorded camera and click, not this hypothesis.
"""
import argparse
import copy
import json
from .conic_centers import quarter


def request():
    line = dict(type="line", start=[2., -2., 0.], end=[8., -2., 0.])
    circle = dict(type="circle", center=[4., -4., 0.], radius=2., normal=[0., 0., 1.], x_axis=[1., 0., 0.])
    cases = [
        ("line", line, [3.5, -2., 0.], [4, 3]),
        ("line-off-plane", dict(line, start=[2., -2., 7.], end=[8., -2., 7.]), [3.5, -2., 7.], [4, 3]),
        ("line-tilted", dict(line, start=[2., -2., 0.], end=[8., -4., 7.]), [3.5, -2.5, 1.75], [4, 3]),
        ("polyline", dict(type="polyline", vertices=[[2., -2., 0.], [8., -2., 0.], [8., -6., 0.]]), [3.5, -2., 0.], [4, 3]),
        ("circle", circle, [2.4, -2.8, 0.], [3, 2]),
        ("ellipse-quarter", dict(quarter(), type="nurbs"), [2.4, -3.4, 0.], [3, 2]),
    ]
    operations = []
    for name, source, aim, offset in cases:
        for modes in [None, ["Near"]]:
            operations.append(operation(name + ("-one-shot" if modes is None else "-persistent"),
                                        [source], aim, offset, modes))
    for name, aim, modes in [
        ("line-end", [2.3, -2., 0.], ["Near", "End"]),
        ("line-mid", [4.95, -2., 0.], ["Near", "Mid"]),
        ("circle-center", [2.4, -2.8, 0.], ["Near", "Cen"]),
        ("circle-quad", [2.05, -3.56, 0.], ["Near", "Quad"]),
    ]:
        operations.append(operation(name, [circle if name.startswith("circle") else line], aim, [0, 0], modes))
    return dict(protocol_version=1, iterations=1, operations=operations)


def operation(name, sources, aim, offset, modes):
    return dict(op="split_edge_command", id=name,
                sources=[dict(brep=dict(source=dict(type="box", min=[0., 0., 0.], max=[10., 12., 14.])))] + copy.deepcopy(sources),
                selected=[0], edge=0, pick="mouse",
                inputs=[dict(pick=dict(point=aim, aim=aim, offset=offset, osnap="Near" if modes is None else "Persistent"))],
                persistent_snaps=["Point", "End", "Mid", "Cen", "Quad"] if modes is None else modes,
                record_viewport=True, undo_redo=True, trace_commands=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", help="owned shared box artifact, for a live Rhino run")
    args = parser.parse_args()
    data = request()
    if args.artifact:
        for item in data["operations"]:
            item["sources"][0]["brep"]["artifact_path"] = args.artifact
    print(json.dumps(data, indent=2, allow_nan=False))
