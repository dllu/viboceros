"""Validation must precede all document and GUI access."""
import unittest
from . import border_probe


class BorderProbeTests(unittest.TestCase):
    def test_brep_sources_require_identical_geometry_before_host_access(self):
        for kind in ("box", "extrusion", "brep", "mesh_brep", "surface_face", "tube"):
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "shared source artifact"):
                border_probe.run({"command": "DupBorder", "source": {"type": kind}}, None, {})

    def test_edge_order_is_a_valid_permutation_before_host_access(self):
        for order in ([1, 1], [-1, 0], [0, 2], [True, 0], [0.5], "0,1"):
            with self.subTest(order=order), self.assertRaisesRegex(ValueError, "permutation"):
                border_probe.run({"command": "DupBorder", "edge_order": order}, None, {})
        border_probe.validate({"command": "DupBorder", "edge_order": [2, 0, 1]})

    def test_defaults_and_face_order(self):
        self.assertEqual(border_probe.validate({"command": "DupBorder"}),
                         ("DupBorder", "Current", False, None))
        self.assertEqual(border_probe.validate({"command": "DupFaceBorder",
                         "faces": [2, 0], "preselect": True, "output_layer": "Input"}),
                         ("DupFaceBorder", "Input", True, [2, 0]))

    def test_invalid_options_cannot_touch_host(self):
        for update in ({"command": "Delete"}, {"output_layer": "Bad"},
                       {"preselect": 1}, {"faces": [0]},
                       {"preselect": True, "faces": []},
                       {"preselect": True, "faces": [0, 0]},
                       {"preselect": True, "faces": [-1]},
                       {"preselect": True, "faces": [True]},
                       {"preselect": True, "faces": [0.5]}):
            with self.subTest(update=update), self.assertRaises(ValueError):
                border_probe.run(dict({"command": "DupBorder"}, **update), None, {})
