# -*- coding: utf-8 -*-
"""Observe Rhino's IntersectSelf output on independent polyline sources."""


def run(operation, host):
    Rhino = host["Rhino"]
    System = host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    if "curve" in operation:
        source = host["_join_close_input"](operation["curve"])
    else:
        points = [Rhino.Geometry.Point3d(*value) for value in operation["vertices"]]
        source = Rhino.Geometry.PolylineCurve(points)
    source_id = document.Objects.AddCurve(source)
    if source_id == System.Guid.Empty:
        source.Dispose()
        raise ValueError("could not add IntersectSelf source")
    results = []
    try:
        api_events = None
        if operation.get("include_api"):
            api_events = []
            events = Rhino.Geometry.Intersect.Intersection.CurveSelf(
                source, document.ModelAbsoluteTolerance)
            if events is not None:
                for event in events:
                    if event.IsOverlap:
                        api_events.append({"type": "overlap",
                                           "a": [float(event.OverlapA.T0), float(event.OverlapA.T1)],
                                           "b": [float(event.OverlapB.T0), float(event.OverlapB.T1)],
                                           "points": [[event.PointA.X, event.PointA.Y, event.PointA.Z],
                                                      [event.PointA2.X, event.PointA2.Y, event.PointA2.Z]]})
                    else:
                        api_events.append({"type": "point", "a": float(event.ParameterA),
                                           "b": float(event.ParameterB),
                                           "point": [event.PointA.X, event.PointA.Y, event.PointA.Z]})
        selected_as_self_intersecting = None
        if operation.get("include_selection"):
            document.Objects.UnselectAll()
            if not Rhino.RhinoApp.RunScript("! _SelSelfIntersectingCrv", True):
                raise ValueError("Rhino SelSelfIntersectingCrv failed")
            selected_as_self_intersecting = bool(
                document.Objects.FindId(source_id).IsSelected(False))
            document.Objects.UnselectAll()
        before = set(obj.Id for obj in document.Objects)
        history_before = Rhino.RhinoApp.CommandHistoryWindowText
        succeeded = bool(Rhino.RhinoApp.RunScript(
            "! _IntersectSelf _SelID {} _Enter".format(source_id), True))
        history = Rhino.RhinoApp.CommandHistoryWindowText
        if history.startswith(history_before):
            history = history[len(history_before):]
        for obj in document.Objects:
            if obj.Id in before:
                continue
            results.append(obj.Id)
        output = []
        for result_id in results:
            obj = document.Objects.FindId(result_id)
            geometry = obj.Geometry
            if isinstance(geometry, Rhino.Geometry.Point):
                point = geometry.Location
                output.append({"type": "point", "point": [point.X, point.Y, point.Z]})
            elif isinstance(geometry, Rhino.Geometry.Curve):
                first = geometry.PointAtStart
                last = geometry.PointAtEnd
                output.append({"type": "curve", "start": [first.X, first.Y, first.Z],
                               "end": [last.X, last.Y, last.Z]})
            else:
                raise ValueError("unexpected IntersectSelf output type")
        output.sort(key=lambda item: repr(item))
        report = [line.strip() for line in history.splitlines()
                  if line.strip().startswith("Found ")]
        if not succeeded or len(report) != 1:
            raise ValueError("Rhino IntersectSelf did not return a result")
        value = {"report": report[0], "output": output,
                 "source_selected": bool(document.Objects.FindId(source_id).IsSelected(False)),
                 "selected_output_count": sum(bool(document.Objects.FindId(result_id).IsSelected(False))
                                              for result_id in results)}
        if api_events is not None:
            value["api_events"] = api_events
        if selected_as_self_intersecting is not None:
            value["selected_as_self_intersecting"] = selected_as_self_intersecting
        return value, 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        for result_id in results:
            document.Objects.Delete(result_id, True)
        document.Objects.Delete(source_id, True)
        source.Dispose()
