import copy
import math
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .untrim_holes_probe import validate
from .untrim_holes_cases import request
from .untrim_holes_capture import capture
from .untrim_holes_undo_cases import request as undo_request
from .untrim_holes_limits_cases import request as limits_request
from .untrim_replay import replay


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
                        dict(undo_after=None), dict(undo_after=[True]), dict(undo_after=[0]),
                        dict(undo_after=[2]), dict(undo_after=[1, 1]), dict(undo_after=[1]),
                        dict(sources=[]), dict(sources=[{}]), dict(op="untrim_all_command")):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(base, **changes))
        no_artifact = copy.deepcopy(base)
        no_artifact["sources"][0]["brep"].pop("artifact_path")
        with self.assertRaises(ValueError): validate(no_artifact)

    def test_internal_undo_requires_owned_mouse_input_and_ordered_pick_checkpoints(self):
        fixture = dict(self.fixture(), pick="mouse", undo_after=[1])
        validate(fixture)
        fixture["components"] = [[0, 3], [0, 4]]
        fixture["undo_after"] = [1, 2]
        validate(fixture)
        for checkpoints in ([2, 1], [-1], [3], [1, 2, 2], [1.]):
            with self.subTest(checkpoints=checkpoints), self.assertRaises(ValueError):
                validate(dict(fixture, undo_after=checkpoints))

    def test_native_internal_undo_restores_trims_and_removes_retained_objects(self):
        root = Path(__file__).parent
        fixture = json.loads((root / "fixtures/untrim_holes_undo.json").read_text())
        observed = json.loads((root / "observations/untrim_holes_undo.json").read_text())
        self.assertEqual(undo_request(), fixture)
        self.assertEqual(len(observed["results"]), 8)
        for operation, row in zip(fixture["operations"], observed["results"]):
            self.assertEqual(operation["id"], row["id"])
            value = row["value"]
            self.assertIn("_Undo", value["history"])
            self.assertEqual(value["succeeded"], operation["finish"] == "Enter")
            repick = len(operation["components"]) == 2
            if not repick:
                self.assertEqual(value["after"], value["before"])
            else:
                self.assertEqual(len(value["after"]), 3 if operation["all"] else 2)
                source = next(obj for obj in value["after"] if obj["source"] == 0)
                self.assertEqual(len(source["geometry"]["definition"]["faces"][0]["loops"]),
                    1 if operation["all"] else 2)

    def test_replay_isolates_source_paths_and_compares_all_geometry_and_metadata(self):
        root = Path(__file__).parent
        fixture = request()
        original = copy.deepcopy(fixture)
        observed = json.loads((root / "observations/untrim_holes_components.json").read_text())
        native = copy.deepcopy(observed)
        native["engine"] = "viboceros"
        client = Mock()
        client.run_viboceros.return_value = native
        self.assertTrue(replay(fixture, observed, client).passed)
        self.assertEqual(fixture, original)

        prepared = client.run_viboceros.call_args.args[0]
        paths = [Path(op["sources"][0]["brep"]["artifact_path"]) for op in prepared["operations"]]
        self.assertEqual(len(set(paths)), 44)
        self.assertTrue(all(not path.parent.exists() for path in paths))
        for field in ("weight", "trim", "color", "groups", "selection"):
            bad = copy.deepcopy(observed)
            obj = next(obj for obj in bad["results"][1]["value"]["after"] if obj["source"] == 0)
            if field == "weight": obj["geometry"]["definition"]["faces"][0]["definition"]["control_points"][0]["weight"] = 2.
            elif field == "trim": obj["geometry"]["definition"]["faces"][0]["loops"][0][0]["iso"] = 1
            elif field == "color": obj["color"][0] = 99
            elif field == "groups": obj["groups"] = []
            else: obj["selected"] = True
            with self.subTest(field=field): self.assertFalse(replay(fixture, bad, client).passed)
        self.assertEqual(fixture, original)

    def test_multi_edge_length_filter_uses_the_complete_hole_perimeter_inclusively(self):
        root = Path(__file__).parent
        fixture = json.loads((root / "fixtures/untrim_holes_limits.json").read_text())
        observed = json.loads((root / "observations/untrim_holes_limits.json").read_text())
        self.assertEqual(limits_request(), fixture)
        self.assertEqual(len(observed["results"]), 42)
        for operation, row in zip(fixture["operations"], observed["results"]):
            self.assertEqual(operation["id"], row["id"])
            perimeter = 8. if operation["id"].startswith("square-") else 10.
            admitted = operation["maximum_edge_length"] == 0. or operation["maximum_edge_length"] >= perimeter
            value = row["value"]
            self.assertTrue(value["succeeded"])
            self.assertEqual(len(value["after"]), 2 if admitted else 1)
            source = next(obj for obj in value["after"] if obj["source"] == 0)
            self.assertEqual(len(source["geometry"]["definition"]["faces"][0]["loops"]),
                1 if admitted else 2)
            if not admitted: self.assertEqual(value["after"], value["before"])


if __name__ == "__main__": unittest.main()
