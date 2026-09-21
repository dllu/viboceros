"""Validate before host access and defer history until the script has returned."""
import unittest
from types import SimpleNamespace
from . import merge_edges_probe
from .test_join_probe import Event


class MergeEdgesProbeTests(unittest.TestCase):
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
