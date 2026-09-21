"""Cap fixture validation cannot touch document or GUI state."""
import unittest
from . import cap_probe


class CapProbeTests(unittest.TestCase):
    def test_requires_shared_artifact_before_host_access(self):
        for path in (None, False, 12, [], ""):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "shared source artifact"):
                cap_probe.run({"artifact_path": path}, None, {})

    def test_invalid_selection_topology_and_booleans_cannot_touch_host(self):
        for update in ({"preselect": 1}, {"reversed": "False"},
                       {"keep_faces": []}, {"keep_faces": [0, 0]},
                       {"keep_faces": [-1]}, {"keep_faces": [True]},
                       {"keep_faces": [0.5]}, {"keep_faces": "0"},
                       {"edge_order": [0, 0]}, {"edge_order": [-1]},
                       {"edge_order": [0, 2]}, {"edge_order": [True]},
                       {"edge_order": "0,1"}):
            with self.subTest(update=update), self.assertRaises(ValueError):
                cap_probe.run(dict({"artifact_path": "owned.3dm"}, **update), None, {})

    def test_valid_face_and_edge_orders_are_not_rewritten(self):
        operation = {"artifact_path": "owned.3dm", "preselect": True,
                     "keep_faces": [2, 0], "edge_order": [1, 0]}
        cap_probe.validate(operation)
        self.assertEqual(operation["keep_faces"], [2, 0])
        self.assertEqual(operation["edge_order"], [1, 0])
