"""Validate before host access and defer history until the script has returned."""
import unittest
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
from . import merge_edges_probe
from .test_join_probe import Event


class MergeEdgesProbeTests(unittest.TestCase):
    def test_retained_mouse_choices_preserve_identity_attributes_and_complete_history(self):
        root = Path(__file__).resolve().parents[2]
        directory = root / "tools/rhino_oracle/diagnostics/merge_edge"
        request = json.loads((directory / "mouse-request.json").read_text())
        observed = json.loads((directory / "mouse-response.json").read_text())
        provenance = json.loads((root / "docs/merge-edge-mouse-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((root / path).read_bytes()).hexdigest(), digest)
        for operation in request["operations"]:
            operation["sources"][0]["brep"]["artifact_path"] = "/owned/source.3dm"
        merge_edges_probe.validate_mouse_request(request)
        self.assertEqual(len(request["operations"]), 15)
        self.assertEqual(len(observed["results"]), 15)
        counts = {choice: 0 for choice in ("EdgeA", "EdgeB", "Both", "All", "Cancel")}
        for operation, result in zip(request["operations"], observed["results"]):
            self.assertEqual(operation["id"], result["id"])
            choice, value = operation["choice"], result["value"]
            counts[choice] += 1
            success = choice != "Cancel"
            self.assertEqual(value["succeeded"], success)
            self.assertEqual(value["history_tested"], success)
            self.assertEqual(len(value["before"]), 1)
            self.assertEqual(len(value["after"]), 1)
            before, after = value["before"][0], value["after"][0]
            self.assertEqual({k: v for k, v in before.items() if k != "geometry"},
                             {k: v for k, v in after.items() if k != "geometry"})
            self.assertFalse(after["selected"])
            self.assertEqual(len(before["geometry"]["brep"]["edges"]) - len(after["geometry"]["brep"]["edges"]),
                             dict(EdgeA=1, EdgeB=1, Both=2, All=3, Cancel=0)[choice])
            events = [e for e in value["command_events"] if e["name"] == "MergeEdge"]
            self.assertEqual(len(events), 1)
            self.assertEqual(events[0]["result"], "Success" if success else "Cancel")
            self.assertEqual(events[0]["objects"], value["after"])
            if success:
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["undo_event_snapshot"], value["undo"])
                self.assertEqual(value["redo_event_snapshot"], value["redo"])
            else:
                self.assertEqual(value["before"], value["after"])
                self.assertNotIn("undo", value)
        self.assertEqual(counts, dict.fromkeys(counts, 3))

    def test_mouse_macros_have_one_bounded_choice_and_no_typed_coordinate_pick(self):
        operation = dict(op="merge_edge_command", id="box", edge=13, pick="mouse",
                         sources=[dict(brep=dict(artifact_path="/owned/source.3dm"))])
        self.assertEqual(merge_edges_probe.mouse_command(operation), "_-MergeEdge _Pause _All _Enter")
        for choice in ("EdgeA", "EdgeB", "Both", "All"):
            self.assertEqual(merge_edges_probe.mouse_command(dict(operation, choice=choice)),
                             "_-MergeEdge _Pause _" + choice + " _Enter")
        for choice, suffix in (("Cancel", "_Cancel"), ("Auto", "_Enter")):
            self.assertEqual(merge_edges_probe.mouse_command(dict(operation, choice=choice)),
                             "_-MergeEdge _Pause " + suffix)
        with self.assertRaises(ValueError):
            merge_edges_probe.mouse_command(dict(operation, choice="All _Delete"))
        with self.assertRaises(ValueError):
            merge_edges_probe.mouse_command(dict(operation, pick="point"))

    def test_mouse_input_requires_bounded_dedicated_request_and_safe_unique_ids(self):
        operation = dict(op="merge_edge_command", id="box-0.seed13", edge=13, pick="mouse",
                         sources=[dict(brep=dict(artifact_path="/owned/source.3dm"))])
        valid = dict(protocol_version=1, operations=[operation])
        merge_edges_probe.validate_mouse_request(valid)
        invalid_requests = [dict(valid, protocol_version=v) for v in (True, 1.0, 2, None)]
        invalid_requests += [dict(valid, iterations=v) for v in (True, 1.0, 0, 2, None)]
        invalid_requests += [dict(valid, operations=v) for v in ([], None, {}, [operation] * 129, [operation] * 2)]
        invalid_operations = [dict(id=v) for v in (None, 1, "", "x" * 101, "bad\n", "bad name", "bad\nPICK injected 0 0")]
        invalid_operations += [dict(op="merge_edges_command"), dict(pick="point"),
                               dict(preselect=True), dict(cancel=True), dict(edge=-1),
                               dict(choice="All\n_Delete"), dict(choice=True), dict(choice=None)]
        for update in invalid_operations:
            request = copy.deepcopy(valid)
            request["operations"][0].update(update)
            invalid_requests.append(request)
        invalid_requests += [dict(valid, operations=[None]), dict(valid, operations=[operation, dict(operation, id="second", pick="point")])]
        for request in invalid_requests:
            with self.subTest(request=request), self.assertRaises(ValueError):
                merge_edges_probe.validate_mouse_request(request)

    def test_invalid_requests_never_access_host(self):
        valid = {"sources": [{"brep": {"artifact_path": "/owned/source.3dm"}}]}
        updates = [{"sources": s} for s in (None, [], {}, [{}] * 33, [None], [{"brep": {}}], [{"type": "unknown"}])]
        updates += [{"selected": s} for s in ([], [True], [0.0], [-1], [1], [0,0], "0")]
        updates += [{key: value} for key in ("preselect", "undo_redo", "cancel", "trace_commands") for value in (0, 1, "Yes", None)]
        updates += [{key: value} for key in ("absolute_tolerance", "angular_tolerance") for value in (True, "1", 0, -1, float("nan"), float("inf"))]
        updates += [{"preselect": True, "cancel": True}]
        updates += [{"op": "merge_edge_command", "edge": edge, "preselect": True} for edge in (None, True, -1, 0.5, "0")]
        updates += [{"edge": 0}]
        updates += [{"op": "merge_edge_command", "preselect": True}]
        updates += [{"op": "merge_edge_command", "edge": 0, "pick": "unknown"}]
        updates += [{"op": "merge_edge_command", "edge": 0, "pick": "point", "preselect": True}]
        updates += [{"op": "merge_edge_command", "edge": 0, "pick": "point", "cancel": True}]
        updates += [{"op": "merge_edge_command", "edge": 0, "pick": "point", "sources": [{"type": "point", "point": [0,0,0]}]}]
        for update in updates:
            with self.subTest(update=update), self.assertRaises(ValueError):
                merge_edges_probe.run(dict(valid, **update), None, {})

    def test_valid_order_and_defaults_are_preserved(self):
        sources = [{"type": "point", "point": [0,0,0]}, {"brep": {"artifact_path": "/owned/source.3dm"}}]
        self.assertEqual(merge_edges_probe.validate({"sources": sources}), (sources, [0,1]))
        self.assertEqual(merge_edges_probe.validate({"sources": sources, "selected": [1,0]}), (sources, [1,0]))
        self.assertEqual(merge_edges_probe.validate({"op": "merge_edge_command", "sources": sources,
            "selected": [1], "edge": 2, "preselect": True}), (sources, [1]))
        self.assertEqual(merge_edges_probe.validate({"op": "merge_edge_command", "sources": sources,
            "selected": [1], "edge": 2, "pick": "point"}), (sources, [1]))
        for choice in ("EdgeA", "EdgeB", "Both", "All", "Cancel", "Auto"):
            self.assertEqual(merge_edges_probe.validate({"op": "merge_edge_command", "sources": sources,
                "selected": [1], "edge": 2, "pick": "mouse", "choice": choice}), (sources, [1]))

    def test_idle_waits_for_command_exit_and_detaches_before_reentrant_callback(self):
        event, in_command, calls = Event(), [True], []
        rhino = SimpleNamespace(RhinoApp=SimpleNamespace(Idle=event),
            Commands=SimpleNamespace(Command=SimpleNamespace(InCommand=lambda: in_command[0])))
        def callback():
            self.assertEqual(event.handlers, [])
            calls.append(1)
            event.fire("idle")
        merge_edges_probe.at_idle(rhino, callback)
        event.fire("idle")
        self.assertEqual(calls, [])
        in_command[0] = False
        event.fire("idle")
        event.fire("idle")
        self.assertEqual(calls, [1])

    def test_idle_detaches_even_when_callback_fails(self):
        event = Event()
        rhino = SimpleNamespace(RhinoApp=SimpleNamespace(Idle=event),
            Commands=SimpleNamespace(Command=SimpleNamespace(InCommand=lambda: False)))
        def fail(): raise RuntimeError("failed")
        merge_edges_probe.at_idle(rhino, fail)
        with self.assertRaisesRegex(RuntimeError, "failed"):
            event.fire("idle")
        self.assertEqual(event.handlers, [])
