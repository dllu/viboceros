import copy
import math
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .untrim_holes_probe import validate
from .untrim_holes_cases import request
from .untrim_holes_capture import capture


class UntrimHolesProbeTests(unittest.TestCase):
    def test_recorded_sources_regenerate_and_native_edits_survive_escape(self):
        root = Path(__file__).parent
        fixture = json.loads((root / "fixtures/untrim_holes_components.json").read_text())
        observed = json.loads((root / "observations/untrim_holes_components.json").read_text())
        self.assertEqual(request(), fixture)
        self.assertEqual(len(observed["results"]), 44)
        values = {row["id"]: row["value"] for row in observed["results"]}
        for all_value in (0, 1):
            enter, cancel = (values["mouse-all-%d-%s" % (all_value, finish)] for finish in ("Enter", "Cancel"))
            self.assertTrue(enter["succeeded"])
            self.assertFalse(cancel["succeeded"])
            self.assertEqual(enter["after"], cancel["after"])
            self.assertNotEqual(cancel["before"], cancel["after"])
            self.assertIn("_Pause", enter["history"])
            self.assertIn("_Cancel", cancel["history"])
        for operation, row in zip(fixture["operations"], observed["results"]):
            self.assertEqual(operation["id"], row["id"])
            for obj in row["value"]["after"]:
                if obj["source"] is None:
                    self.assertTrue(operation["keep_trim_objects"])
                    self.assertIsNone(obj["name"])
                    self.assertEqual(obj["groups"], [])
                    self.assertEqual(obj["color_source"], "ColorFromLayer")
                    self.assertFalse(obj["selected"])
                    self.assertTrue(obj["current_layer"])
        for all_value in (0, 1):
            for maximum, loops in (("0.0", 1), ("7.9", 2), ("8.0", 1), ("8.1", 1)):
                value = values["length-%s-all-%d" % (maximum, all_value)]
                self.assertEqual(len(value["after"][0]["geometry"]["definition"]["faces"][0]["loops"]), loops)
        retained = values["tube-solid-all-all-0-keep-1"]["after"][0]["geometry"]
        self.assertEqual(retained["type"], "brep")
        self.assertEqual(len(retained["definition"]["faces"]), 1)

    def test_capture_exports_only_source_recipes_to_owned_paths(self):
        fixture = request()
        original = copy.deepcopy(fixture)
        client = Mock()
        paths = []
        def export(prepared, timeout):
            for operation in prepared["operations"]:
                self.assertEqual(operation["op"], "brep_remove_holes")
                self.assertEqual(operation["loops"], [])
                path = Path(operation["source"]["artifact_path"])
                self.assertTrue(path.parent.is_dir())
                self.assertFalse(path.exists())
                paths.append(path)
        client.run_viboceros.side_effect = export
        client.run_rhino.return_value = {"native": "response"}
        self.assertEqual(capture(fixture, client), {"native": "response"})
        self.assertEqual(fixture, original)
        self.assertEqual(len(paths), 44)
        self.assertEqual(len(set(paths)), 44)
        self.assertTrue(all(not path.parent.exists() for path in paths))
        client.run_rhino.assert_called_once()
        client.reset_mock()
        with self.assertRaises(ValueError): capture(dict(fixture, iterations=2), client)
        client.run_viboceros.assert_not_called()
        client.run_rhino.assert_not_called()

    def fixture(self):
        return dict(op="untrim_holes_command", id="owned", sources=[dict(brep=dict(
            source=dict(type="solid_tube", radii=[2., 5.], height=8.), artifact_path="/owned/source.3dm"))],
            all=False, components=[[0, 3]], maximum_edge_length=0., keep_trim_objects=False, pick="preselect")

    def test_supported_component_workflows_and_finish_modes(self):
        for all_value in (False, True):
            for pick in ("preselect", "mouse"):
                for finish in ("Enter", "Cancel"):
                    fixture = dict(self.fixture(), all=all_value, pick=pick, finish=finish)
                    validate(fixture)

    def test_bad_component_options_are_rejected_before_document_edits(self):
        base = self.fixture()
        for changes in (dict(all=1), dict(keep_trim_objects=1), dict(components=None),
                        dict(components=[[1, 3]]), dict(components=[[0, -1]]),
                        dict(components=[[False, 0]]), dict(components=[[0, 1]] * 65),
                        dict(components=[[0, 3, 1]]), dict(pick="point"), dict(finish="Delete"),
                        dict(id="x _Delete"), dict(source_layer=1), dict(extra=1),
                        dict(maximum_edge_length=-1), dict(maximum_edge_length=True),
                        dict(maximum_edge_length=math.nan), dict(maximum_edge_length=math.inf),
                        dict(maximum_edge_length=10 ** 400),
                        dict(sources=[]), dict(sources=[{}]), dict(op="untrim_all_command")):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(base, **changes))
        no_artifact = copy.deepcopy(base)
        no_artifact["sources"][0]["brep"].pop("artifact_path")
        with self.assertRaises(ValueError): validate(no_artifact)


if __name__ == "__main__": unittest.main()
