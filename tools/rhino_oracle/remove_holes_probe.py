"""Public Brep.RemoveHoles API on identical owned sources, without document edits.

This records geometry API behavior; it does not run the UntrimHoles command.
"""
import re


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) != {"op", "id", "source", "loops"}
            or operation.get("op") != "brep_remove_holes"
            or not isinstance(operation.get("id"), str)
            or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", operation["id"]) is None
            or not isinstance(operation.get("source"), dict)):
        raise ValueError("invalid hole removal fixture")
    path = operation["source"].get("artifact_path")
    if not isinstance(path, (str, type(u""))) or not path:
        raise ValueError("hole removal requires a shared source artifact")
    loops = operation["loops"]
    if loops is not None and (not isinstance(loops, list) or len(loops) > 1000
            or any(not isinstance(pair, list) or len(pair) != 2
                or any(type(index) is not int or index < 0 for index in pair)
                for pair in loops)):
        raise ValueError("hole removal requires face/loop indices or null for all")
    return path, loops


def run(operation, tolerance, host):
    path, loops = validate(operation)
    Rhino = host["Rhino"]
    if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
    model = Rhino.FileIO.File3dm.Read(path)
    if model is None: raise ValueError("cannot read hole removal source")
    source, result, snapshot = None, None, None
    try:
        try:
            entries = list(model.Objects)
            if len(entries) != 1: raise ValueError("hole removal artifact needs one object")
            source = entries[0].Geometry.Duplicate()
        finally: model.Dispose()
        if not isinstance(source, Rhino.Geometry.Brep) or not source.IsValid:
            raise ValueError("invalid hole removal source")
        record = lambda g: host["_interchange_brep_record"](g, include_samples=False)
        before = record(source)
        if loops is None:
            result = source.RemoveHoles(float(tolerance["absolute"]))
        else:
            components = []
            for face, loop in loops:
                if face >= source.Faces.Count or loop >= source.Faces[face].Loops.Count:
                    raise ValueError("hole removal loop index outside source")
                boundary = source.Faces[face].Loops[loop]
                components.append(Rhino.Geometry.ComponentIndex(
                    Rhino.Geometry.ComponentIndexType.BrepLoop, boundary.LoopIndex))
            # Explicit typing also resolves the empty collection overload.
            collection = host["System"].Array[Rhino.Geometry.ComponentIndex](components)
            result = source.RemoveHoles(collection, float(tolerance["absolute"]))
        after = None
        if result is not None:
            snapshot = result.DuplicateBrep()
            if not snapshot.IsValid: raise ValueError("invalid hole removal result")
            after = record(snapshot)
        if record(source) != before: raise ValueError("hole removal mutated its source")
        return dict(before=before, after=after), 0
    finally:
        if snapshot is not None: snapshot.Dispose()
        if result is not None: result.Dispose()
        if source is not None: source.Dispose()
