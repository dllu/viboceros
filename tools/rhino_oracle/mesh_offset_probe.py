# -*- coding: utf-8 -*-
"""Observe public Rhino Mesh.Offset and the OffsetMesh command on owned meshes."""


def run(operation, host):
    Rhino = host["Rhino"]
    System = host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    source = host["_polygon_mesh"](operation["vertices"], operation["faces"])
    api_output = None
    api = None
    source_id = None
    results = []
    try:
        if operation.get("include_api", True):
            api = source.Offset(float(operation["distance"]), bool(operation.get("solid", False)))
            if api is None:
                raise ValueError("Rhino Mesh.Offset returned no mesh")
            api_output = host["_polygon_mesh_value"](api)
        source_id = document.Objects.AddMesh(source)
        if source_id == System.Guid.Empty:
            raise ValueError("could not add OffsetMesh source")
        before = set(obj.Id for obj in document.Objects)
        history_before = Rhino.RhinoApp.CommandHistoryWindowText
        macro = "! _OffsetMesh _SelID {} _Enter {}".format(source_id, operation["macro"])
        succeeded = bool(Rhino.RhinoApp.RunScript(macro, True))
        history = Rhino.RhinoApp.CommandHistoryWindowText
        if history.startswith(history_before):
            history = history[len(history_before):]
        for obj in document.Objects:
            if obj.Id not in before:
                results.append(obj.Id)
        if not succeeded or not results:
            raise ValueError("Rhino OffsetMesh produced no result: " + history[-1200:])
        output = []
        for result_id in results:
            obj = document.Objects.FindId(result_id)
            if not isinstance(obj.Geometry, Rhino.Geometry.Mesh):
                raise ValueError("OffsetMesh produced non-mesh geometry")
            output.append(host["_polygon_mesh_value"](obj.Geometry))
        value = {"api": api_output, "output": output,
                 "source_exists": document.Objects.FindId(source_id) is not None}
        if operation.get("include_history", False):
            value["history"] = history
        return value, 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        for result_id in results:
            document.Objects.Delete(result_id, True)
        if source_id is not None:
            document.Objects.Delete(source_id, True)
        if api is not None:
            api.Dispose()
        source.Dispose()
