"""Public-API unit-change probe for a dedicated, isolated Rhino 8 instance.

Run a copy in an empty job directory. Writes response.json and exits Rhino.
Every test document is headless and disposed; the active document is untouched.
This measures AdjustModelUnitSystem, not the interactive Units command.
"""
import json
import os
import Rhino
import System


def snapshot(document, ids):
    objects = []
    for object_id in ids:
        obj = document.Objects.FindId(object_id)
        if obj is None:
            raise ValueError("unit change lost an object")
        point = obj.Geometry.Location
        objects.append({"point": [point.X, point.Y, point.Z],
                        "mode": str(obj.Attributes.Mode),
                        "selected": bool(obj.IsSelected(False))})
    return {"units": int(document.ModelUnitSystem),
            "absolute": document.ModelAbsoluteTolerance,
            "relative": document.ModelRelativeTolerance,
            "angular": document.ModelAngleToleranceRadians,
            "objects": objects}


def generate_case(source, target, scale):
    if type(source) is not int or type(target) is not int or source not in (0, 2, 4, 8) or target not in (0, 2, 4, 8):
        raise ValueError("unsupported unit code")
    if type(scale) is not bool:
        raise ValueError("rescale must be boolean")
    document = Rhino.RhinoDoc.CreateHeadless(None)
    try:
        document.ModelUnitSystem = System.Enum.ToObject(Rhino.UnitSystem, source)
        document.ModelAbsoluteTolerance = 0.001
        document.ModelRelativeTolerance = 0.0001
        document.ModelAngleToleranceRadians = 0.00001
        ids = []
        for x, mode in [(1000.0, Rhino.DocObjects.ObjectMode.Normal),
                        (500.0, Rhino.DocObjects.ObjectMode.Hidden),
                        (250.0, Rhino.DocObjects.ObjectMode.Locked)]:
            attributes = Rhino.DocObjects.ObjectAttributes()
            try:
                attributes.Mode = mode
                object_id = document.Objects.AddPoint(Rhino.Geometry.Point3d(x, 2*x, 3*x), attributes)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add probe point")
                ids.append(object_id)
            finally:
                attributes.Dispose()
        document.Objects.Select(ids[0])
        before = snapshot(document, ids)
        document.AdjustModelUnitSystem(System.Enum.ToObject(Rhino.UnitSystem, target), scale)
        after = snapshot(document, ids)
        if after["units"] != target:
            raise ValueError("unit change did not apply")
        return {"before": before, "after": after}
    finally:
        document.Dispose()


def generate():
    results = []
    for source, target, scale in [(2, 4, False), (2, 4, True),
                                  (2, 8, False), (2, 8, True),
                                  (8, 2, False), (8, 2, True),
                                  (0, 2, True), (2, 0, True)]:
        results.append({"id": "%d-to-%d-scale-%s" % (source, target, str(scale).lower()),
                        "elapsed_ns": 0, "value": generate_case(source, target, scale)})
    return results


if __name__ == "__main__":
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "response.json")
    if os.path.exists(path):
        raise ValueError("refusing to replace an existing response")
    response = {"protocol_version": 1, "engine": "rhino", "engine_version": str(Rhino.RhinoApp.Version), "iterations": 1, "results": []}
    try:
        response["results"] = generate()
    except Exception as error:
        response["error"] = "%s: %s" % (type(error).__name__, error)
    with open(path + ".tmp", "w") as stream:
        json.dump(response, stream, indent=2, allow_nan=False)
    os.rename(path + ".tmp", path)
    Rhino.RhinoApp.Exit(False)
