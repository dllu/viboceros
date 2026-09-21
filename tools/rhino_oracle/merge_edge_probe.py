# -*- coding: utf-8 -*-
"""Public selected-edge API on one shared B-rep; no document mutations."""
import math


def validate(operation):
    source = operation.get("source")
    if not isinstance(source, dict):
        raise ValueError("selected edge merge requires a source")
    path = source.get("artifact_path")
    if not isinstance(path, (str, type(u""))) or not path:
        raise ValueError("selected edge merge requires a shared artifact")
    edge, angle = operation.get("edge"), operation.get("angle")
    if type(edge) is not int or edge < 0:
        raise ValueError("selected edge merge requires a nonnegative edge index")
    if (type(angle) not in (int, float) or math.isnan(angle) or
            math.isinf(angle) or not 0 <= angle <= math.pi):
        raise ValueError("selected edge merge requires an angle in [0, pi]")
    return path, edge, float(angle)


def run(operation, tolerance, host):
    path, edge, angle = validate(operation)
    import brep_join_probe
    Rhino = host["Rhino"]
    if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
    model = Rhino.FileIO.File3dm.Read(path)
    if model is None: raise ValueError("cannot read selected edge merge source")
    source = None
    try:
        entries = list(model.Objects)
        if len(entries) != 1: raise ValueError("selected edge merge artifact needs one object")
        source = entries[0].Geometry.Duplicate()
    finally: model.Dispose()
    result, snapshot = None, None
    try:
        if not isinstance(source, Rhino.Geometry.Brep) or not source.IsValid:
            raise ValueError("invalid selected edge merge source")
        if edge >= source.Edges.Count: raise ValueError("selected edge index outside source")
        before = brep_join_probe.geometry_record(source, tolerance, host)
        result = source.DuplicateBrep()
        api_return = result.Edges.MergeEdge(edge, angle)
        result.Compact()
        if not result.IsValid: raise ValueError("invalid selected edge merge output")
        # Mutation/compaction invalidates managed component wrappers. Read a
        # fresh deep copy, without fitting, rebuilding or normalizing geometry.
        snapshot = result.DuplicateBrep()
        after = brep_join_probe.geometry_record(snapshot, tolerance, host)
        if brep_join_probe.geometry_record(source, tolerance, host) != before:
            raise ValueError("selected edge merge mutated its independent input")
        return dict(before=before, after=after,
            removed=len(before["edges"]) - len(after["edges"]), api_return=int(api_return)), 0
    finally:
        if snapshot is not None: snapshot.Dispose()
        if result is not None: result.Dispose()
        if source is not None: source.Dispose()
