"""Strict SplitEdge input validation, before any Rhino access or mouse input."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from types import SimpleNamespace
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
