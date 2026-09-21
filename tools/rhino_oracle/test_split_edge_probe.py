"""Strict SplitEdge input validation, before any Rhino access or mouse input."""
import copy
import hashlib
import json
import math
import sys
from pathlib import Path
import unittest
from types import SimpleNamespace
from unittest.mock import mock_open, patch
from . import merge_edges_probe, split_edge_probe


class SplitEdgeProbeTests(unittest.TestCase):
    def test_retained_records_preserve_batch_failure_and_unchanged_replacement_history(self):
        root = Path(__file__).resolve().parents[2]
        request = json.loads((root / "tools/rhino_oracle/fixtures/split_edge_command.json").read_text())
        observed = json.loads((root / "tools/rhino_oracle/observations/split_edge_command.json").read_text())
        metadata = json.loads((root / "docs/split-edge-provenance.json").read_text())
        for path, digest in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((root / path).read_bytes()).hexdigest(), digest)
        self.assertEqual(len(request["operations"]),21)
        self.assertEqual(len(observed["results"]),21)
        counts = dict(success=0, unchanged_replacements=0)
        for operation, result in zip(request["operations"], observed["results"]):
            operation["sources"][0]["brep"]["artifact_path"] = "/owned/source.3dm"
            split_edge_probe.validate(operation)
            self.assertEqual(operation["id"], result["id"])
            value = result["value"]
            before = copy.deepcopy(value["before"])
            before[0]["selected"] = False
            self.assertFalse(value["after"][0]["selected"])
            self.assertEqual(value["succeeded"], value["history_tested"])
            if value["history_tested"]:
                counts["success"] += 1
                counts["unchanged_replacements"] += int(value["before"][0]["geometry"] == value["after"][0]["geometry"])
                self.assertEqual(value["undo"], before)
                self.assertEqual(value["redo"], value["after"])
            else:
                self.assertEqual(value["after"], before)
        self.assertEqual(counts, dict(success=16, unchanged_replacements=4))

    def operation(self):
        return dict(op="split_edge_command", id="split-box", edge=0, pick="mouse",
                    sources=[dict(brep=dict(artifact_path="/owned/source.3dm"))], parameters=[0.25])

    def test_validates_before_host_access(self):
        updates = [dict(parameters=p) for p in (None, {}, "0.25", [True], ["1"], [float("inf")],
                                               [float("nan")], [0.25] * 65)]
        updates += [dict(pick="point"), dict(op="merge_edge_command"), dict(edge=True),
                    dict(choice="All"), dict(cancel=False), dict(preselect=False),
                    dict(finish="Enter _Delete"), dict(object_preselect=1)]
        for update in updates:
            with self.subTest(update=update), self.assertRaises(ValueError):
                split_edge_probe.run(dict(self.operation(), **update), None, {})

    def test_mouse_request_accepts_split_with_unique_bounded_ids(self):
        operation = self.operation()
        request = dict(protocol_version=1, iterations=1, operations=[operation])
        before = copy.deepcopy(request)
        merge_edges_probe.validate_mouse_request(request)
        self.assertEqual(request, before)
        with self.assertRaises(ValueError):
            merge_edges_probe.validate_mouse_request(dict(request, operations=[operation, operation]))

    def test_macro_uses_evaluated_points_and_explicit_finish(self):
        curve = SimpleNamespace(Domain=SimpleNamespace(T0=0, T1=1), PointAt=lambda t: [t, 2, 3])
        host = dict(_xyz=lambda p: p, _command_point=lambda p: "w" + ",".join(map(str, p)))
        for finish in ("Enter", "Cancel"):
            operation = dict(self.operation(), finish=finish, parameters=[0.25, 0.75])
            self.assertEqual(split_edge_probe.mouse_command(operation, curve, host),
                             "_SplitEdge _Pause w0.25,2,3 w0.75,2,3 _" + finish)
        self.assertEqual(split_edge_probe.mouse_command(dict(self.operation(), parameters=[]), curve, host),
                         "_SplitEdge _Pause _Enter")
        with self.assertRaises(ValueError):
            split_edge_probe.mouse_command(dict(self.operation(), parameters=[2]), curve, host)

    def test_distance_inputs_are_exclusive_finite_bounded_and_injection_free(self):
        base = self.operation()
        del base["parameters"]
        valid = [dict(point=0), dict(distance=-2), dict(mouse=0.75), dict(distance=0)]
        split_edge_probe.validate(dict(base, inputs=valid))
        invalid = [None, {}, "0", [None], [{}], [dict(point=1, distance=2)],
                   [dict(command="_Delete")], [dict(mouse="_Enter")], [dict(distance=True)],
                   [dict(distance=float("inf"))], [dict(point=float("nan"))],
                   [dict(distance=10**400)], valid * 17]
        for inputs in invalid:
            with self.subTest(inputs=inputs), self.assertRaises(ValueError):
                split_edge_probe.run(dict(base, inputs=inputs), None, {})
        with self.assertRaises(ValueError):
            split_edge_probe.run(dict(base, parameters=[], inputs=valid), None, {})
        curve = SimpleNamespace(Domain=SimpleNamespace(T0=0, T1=1), PointAt=lambda t: [t, 2, 3])
        host = dict(_xyz=lambda p: p, _command_point=lambda p: "w" + ",".join(map(str, p)))
        self.assertEqual(split_edge_probe.mouse_command(dict(base, inputs=valid), curve, host),
                         "_SplitEdge _Pause w0.0,2,3 -2 _Pause 0 _Enter")
        with self.assertRaises(ValueError):
            split_edge_probe.mouse_command(dict(base, inputs=[dict(mouse=2)]), curve, host)

    def exercise_mouse_driver(self, failure=None):
        class Event:
            def __init__(self): self.handlers = []
            def __iadd__(self, handler): self.handlers.append(handler); return self
            def __isub__(self, handler): self.handlers.remove(handler); return self
            def fire(self):
                for handler in self.handlers: handler(None, None)
        class Timer:
            def __init__(self): self.Tick, self.active, self.disposed = Event(), False, False
            def Start(self): self.active = True
            def Stop(self): self.active = False
            def Dispose(self): self.disposed = True
        timer = Timer()
        app = SimpleNamespace(CommandHistoryWindowText="prior command _Pause\n")
        pixel = lambda x, y: SimpleNamespace(X=x, Y=y)
        viewport = SimpleNamespace(Size=SimpleNamespace(Width=500, Height=500),
                                   WorldToClient=lambda t: pixel(100 + t, 200))
        view = SimpleNamespace(ActiveViewport=viewport, ClientToScreen=lambda p: pixel(p.X+5, p.Y+7))
        rhino = SimpleNamespace(RhinoApp=app, RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(
            Views=SimpleNamespace(ActiveView=view))))
        system = SimpleNamespace(Drawing=SimpleNamespace(Point=pixel))
        operation = dict(id="owned", inputs=[dict(point=0), dict(distance=2), dict(mouse=8), dict(mouse=6)])
        output = mock_open()
        if failure == "write": output.side_effect = OSError("cannot request input")
        def run(script, echo):
            self.assertEqual((script, echo), ("bounded macro", True))
            self.assertTrue(timer.active)
            self.assertEqual(timer.Interval, 100)
            timer.Tick.fire()
            app.CommandHistoryWindowText += "_SplitEdge _Pause\n"
            timer.Tick.fire()
            self.assertEqual(output.call_count, 0)  # Only the component-selection Pause.
            if failure == "history": app.CommandHistoryWindowText = "replaced history _Pause _Pause"
            else: app.CommandHistoryWindowText += "point prompt _Pause\n"
            timer.Tick.fire()
            if failure in ("write", "history"):
                self.assertFalse(timer.active)
                return True
            self.assertEqual(output.call_count, 1)
            timer.Tick.fire()
            self.assertEqual(output.call_count, 1)  # No duplicate click at the same prompt.
            if failure == "script": raise ValueError("command failed")
            if failure == "incomplete": return True
            app.CommandHistoryWindowText += "next point prompt _Pause\n"
            timer.Tick.fire()
            timer.Tick.fire()
            self.assertEqual(output.call_count, 2)
            return True
        host = dict(Rhino=rhino, System=system, __file__="/owned/worker.py", _run_surface_script=run)
        modules = {"clr": SimpleNamespace(AddReference=lambda name: None),
                   "System.Windows.Forms": SimpleNamespace(Timer=lambda: timer)}
        with patch.dict(sys.modules, modules), patch("builtins.open", output):
            if failure:
                with self.assertRaises(ValueError):
                    split_edge_probe.drive(operation, "bounded macro", SimpleNamespace(PointAt=lambda t: t), host)
            else:
                self.assertTrue(split_edge_probe.drive(operation, "bounded macro", SimpleNamespace(PointAt=lambda t: t), host))
                self.assertEqual([call.args[0] for call in output().write.call_args_list],
                                 ["PICK @split:owned:0 113 207\n", "PICK @split:owned:1 111 207\n"])
        self.assertFalse(timer.active)
        self.assertTrue(timer.disposed)
        self.assertEqual(timer.Tick.handlers, [])

    def test_mouse_driver_waits_for_each_prompt_and_never_repeats_clicks(self):
        self.exercise_mouse_driver()

    def test_mouse_driver_disposes_timer_on_command_io_and_history_failures(self):
        for failure in ("script", "incomplete", "write", "history"):
            with self.subTest(failure=failure): self.exercise_mouse_driver(failure)

    def test_mouse_driver_without_extra_clicks_does_not_load_forms(self):
        viewport = SimpleNamespace()
        host = dict(Rhino=SimpleNamespace(RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(
            Views=SimpleNamespace(ActiveView=SimpleNamespace(ActiveViewport=viewport))))),
            System=None, _run_surface_script=lambda script, echo: (script, echo))
        with patch.dict(sys.modules, {"clr": None, "System.Windows.Forms": None}):
            self.assertEqual(split_edge_probe.drive(dict(parameters=[0.25]), "macro", None, host), ("macro", True))

    def test_retained_distances_preserve_raw_geometry_history_and_measured_policy(self):
        root = Path(__file__).resolve().parents[2]
        request = json.loads((root / "tools/rhino_oracle/fixtures/split_edge_distance_command.json").read_text())
        observed = json.loads((root / "tools/rhino_oracle/observations/split_edge_distance_command.json").read_text())
        metadata = json.loads((root / "docs/split-edge-distance-provenance.json").read_text())
        for path, digest in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((root / path).read_bytes()).hexdigest(), digest)
        self.assertEqual(len(request["operations"]), 19)
        self.assertEqual(len(observed["results"]), 19)
        expected_counts = [1,1,2,2,2,1,2, 0,2,2,2,1,2,1, 1,2,2,1,2]
        for operation, result, count in zip(request["operations"], observed["results"], expected_counts):
            self.assertEqual(operation["id"], result["id"])
            operation["sources"][0]["brep"]["artifact_path"] = "/owned/source.3dm"
            split_edge_probe.validate(operation)
            value = result["value"]
            self.assertTrue(value["succeeded"])
            self.assertTrue(value["history_tested"])
            before, after = value["before"][0], value["after"][0]
            before_vertices = before["geometry"]["brep"]["vertices"]
            after_vertices = after["geometry"]["brep"]["vertices"]
            self.assertEqual(after_vertices[:len(before_vertices)], before_vertices)
            self.assertEqual(len(after_vertices)-len(before_vertices), count)
            self.assertEqual({k:v for k,v in before.items() if k != "geometry"},
                             {k:v for k,v in after.items() if k != "geometry"})
            self.assertEqual(value["undo"], value["before"])
            self.assertEqual(value["redo"], value["after"])
            self.assertEqual(value["command_history"].count("_Pause"),
                             1 + sum("mouse" in step for step in operation["inputs"]))

    def test_curved_observations_measure_arc_distance_not_chord_and_retain_residuals(self):
        root = Path(__file__).resolve().parents[2]
        rows = json.loads((root / "tools/rhino_oracle/observations/split_edge_distance_command.json").read_text())["results"]
        points = {r["id"]:r["value"]["after"][0]["geometry"]["brep"]["vertices"] for r in rows}
        primitive = lambda u: (u*math.hypot(1,u)+math.asinh(u))/2
        for name, anchor, distance in [("quadratic-distance",0,10), ("quadratic-interior-distance",0.5,5)]:
            x,y,_ = points[name][4]
            actual = 15 * abs(primitive(1-2*anchor)-primitive(1-2*x/30))
            self.assertGreater(abs(actual-distance), 1e-9)  # Do not pretend the saved positions are exact.
            self.assertLess(abs(actual-distance), 1e-6)
            self.assertGreater(abs(math.hypot(x-30*anchor,y-30*anchor*(1-anchor))-distance), 0.01)
        circle_targets = [("circle-from-seam",2,1), ("circle-forward-wrap",3,2-math.pi/2),
                          ("circle-backward-wrap",2,math.pi/2-2), ("circle-repeat-distance",2,2)]
        for name,index,angle in circle_targets:
            x,y,_ = points[name][index]
            residual = 10*abs(math.atan2(y,x)-angle)
            self.assertGreater(residual, 1e-9)
            self.assertLess(residual, 1e-6)
