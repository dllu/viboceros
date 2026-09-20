"""Validation must reject malformed fixtures before accessing the Rhino host."""
import unittest
from . import mesh_join_probe


class MeshJoinProbeTests(unittest.TestCase):
    def test_invalid_selections_never_touch_host(self):
        for selected in ([], [0, 0], [1], [-1], [True], [0.0], "0"):
            with self.subTest(selected=selected), self.assertRaisesRegex(ValueError, "selection"):
                mesh_join_probe.run({"sources": [{}], "selected": selected}, None, {})

    def test_invalid_options_and_source_counts_never_touch_host(self):
        for sources in ([], [{}] * 33, {}):
            with self.subTest(sources=sources), self.assertRaisesRegex(ValueError, "sources"):
                mesh_join_probe.run({"sources": sources}, None, {})
        for key in ("join_disjoint", "preselect"):
            for value in (0, 1, "Yes", None):
                with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, "boolean"):
                    mesh_join_probe.run({"sources": [{}], key: value}, None, {})
        for value in (0, -1, float("nan"), float("inf"), True, "0.1"):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "tolerance"):
                mesh_join_probe.run({"sources": [{}], "absolute_tolerance": value}, None, {})

    def test_valid_order_and_defaults_are_preserved(self):
        sources = [{}, {}, {}]
        self.assertEqual(mesh_join_probe.validate({"sources": sources}), (sources, [0, 1, 2]))
        self.assertEqual(mesh_join_probe.validate({"sources": sources, "selected": [2, 0],
            "join_disjoint": True, "preselect": False, "absolute_tolerance": 0.125}), (sources, [2, 0]))
