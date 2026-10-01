import copy
import json
from pathlib import Path
import unittest

from .client import _owned_artifact_request, compare_responses
from .remove_holes_cases import request
from .remove_holes_probe import validate


class RemoveHolesTests(unittest.TestCase):
    def captures(self):
        root = Path(__file__).parent
        return (json.loads((root / "fixtures/brep_remove_holes.json").read_text()),
                json.loads((root / "observations/brep_remove_holes.json").read_text()))

    def test_sources_regenerate_and_native_topology_distinguishes_api_overloads(self):
        fixture, observed = self.captures()
        self.assertEqual(request(), fixture)
        self.assertEqual(len(observed["results"]), 28)
        values = {row["id"]: row["value"] for row in observed["results"]}
        for name in ("outer-empty", "outer-outer", "two-holes-empty", "tube-wall-outer"):
            self.assertIsNone(values[name]["after"])
        for name in ("outer-all", "paraboloid-oblique-all"):
            self.assertEqual(values[name]["before"], values[name]["after"])
        for name in ("tube-solid", "tube-open"):
            self.assertEqual(values[name + "-first"], values[name + "-all"])
        selected = values["two-holes-first"]
        before, after = selected["before"], selected["after"]
        self.assertEqual(before["faces"][0]["definition"], after["faces"][0]["definition"])
        self.assertEqual(before["faces"][0]["loops"][0], after["faces"][0]["loops"][0])
        self.assertEqual(before["faces"][0]["loops"][2], after["faces"][0]["loops"][1])
        self.assertEqual(before["edges"][2], after["edges"][1])

    def test_artifacts_are_owned_and_invalid_selection_fails_before_launch(self):
        fixture, _ = self.captures()
        original = copy.deepcopy(fixture)
        with _owned_artifact_request(fixture) as prepared:
            paths = []
            for operation in prepared["operations"]:
                validate(operation)
                paths.append(Path(operation["source"]["artifact_path"]))
            self.assertEqual(len(set(paths)), 28)
            self.assertTrue(all(p.parent.is_dir() and not p.exists() for p in paths))
            base = prepared["operations"][0]
            for changes in (dict(loops=True), dict(loops=[[True, 1]]), dict(loops=[[-1, 1]]),
                            dict(loops=[[0]]), dict(loops=[[0, 1]] * 1001), dict(extra=1),
                            dict(id="x _Delete"), dict(source={})):
                with self.subTest(changes=changes), self.assertRaises(ValueError):
                    validate(dict(base, **changes))
        self.assertEqual(fixture, original)
        self.assertTrue(all(not p.parent.exists() for p in paths))

    def test_full_geometry_comparison_detects_control_net_and_topology_changes(self):
        _, observed = self.captures()
        native = copy.deepcopy(observed)
        native["engine"] = "viboceros"
        self.assertTrue(compare_responses(native, observed, 1e-9, 0).passed)
        for change in ("weight", "surface", "loop"):
            corrupt = copy.deepcopy(observed)
            geometry = corrupt["results"][5]["value"]["after"]
            if change == "weight": geometry["edges"][0]["curve"]["definition"]["control_points"][0]["weight"] += .01
            elif change == "surface": geometry["faces"][0]["definition"]["control_points"][0]["point"][2] += .01
            else: geometry["topology"]["faces"][0]["loops"].pop()
            self.assertFalse(compare_responses(native, corrupt, 1e-9, 0).passed)


if __name__ == "__main__": unittest.main()
