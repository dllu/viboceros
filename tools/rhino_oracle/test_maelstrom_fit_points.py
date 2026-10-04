"""Retained native FitPoints workflows, including unsuccessful command driving."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .maelstrom_fit_points_probe import request, validate

ROOT = Path(__file__).resolve().parents[2]


def read(folder, name):
    return json.loads((ROOT / "tools/rhino_oracle" / folder / (name + ".json")).read_text())


def objects(rows, selection=True):
    # Replacing geometry changes runtime ordering; retain the recorded identity.
    result = {}
    for row in rows:
        key = ("target", row["target"]) if row["target"] is not None else ("definition", row["definition"])
        if key[1] is None or key in result:
            raise AssertionError("missing or repeated owned object identity")
        result[key] = row if selection else dict(row, selected=False)
    return result


class MaelstromFitPointsTests(TestCase):
    def test_factory_closed_inputs_and_native_record_ids(self):
        fixture = read("fixtures", "maelstrom_fit_points")
        result = read("observations", "maelstrom_fit_points")
        self.assertEqual(fixture, request())
        self.assertEqual(len(fixture["operations"]), 17)
        self.assertEqual(result["engine_version"], "8.32.26160.13001")
        self.assertEqual(result["iterations"], 1)
        self.assertEqual([op["id"] for op in fixture["operations"]], [row["id"] for row in result["results"]])
        for op in fixture["operations"]:
            validate(op)
        base = fixture["operations"][0]
        for changes in (dict(op="maelstrom_points"), dict(id="x\n_Exit"), dict(extra=True),
                        dict(points=[]), dict(points=[[0., 0., 0.]]*65), dict(points=[[True, 0., 0.]]*3),
                        dict(points=[[101., 0., 0.]]*3), dict(points=[[float("nan"), 0., 0.]]*3),
                        dict(normal=[0., 0., 0.]), dict(normal=[0., 0.]), dict(normal=[0., 0., float("inf")]),
                        dict(target=0.), dict(target=True), dict(target=101.), dict(target=float("nan")),
                        dict(target=float("inf")), dict(degrees=True), dict(degrees="90 _Exit"),
                        dict(degrees=float("nan")), dict(degrees=float("inf")), dict(degrees=721.),
                        dict(target_kind="_Delete"), dict(target_kind=True)):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(base, **changes))

    def test_private_display_scheme_and_one_iteration_before_launch(self):
        for scheme, count, display, headless in ((None, 1, ":301", ":301"),
                ("VibocerosOracleFitPoints", True, ":301", ":301"),
                ("VibocerosOracleFitPoints", 2, ":301", ":301"),
                ("VibocerosOracleFitPoints", 1, ":0", None),
                ("VibocerosOracleFitPoints", 1, ":301", ":302")):
            q = request()
            q["iterations"] = count
            env = {"DISPLAY": display}
            if headless:
                env["VIBOCEROS_ORACLE_HEADLESS"] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_completed_commands_use_the_sdk_circle_and_point_maps(self):
        compared = 0
        for op, row in zip(read("fixtures", "maelstrom_fit_points")["operations"],
                           read("observations", "maelstrom_fit_points")["results"]):
            v = row["value"]
            event = [e for e in v["events"] if e["name"] == "Maelstrom"]
            self.assertEqual(len(event), 1)
            self.assertEqual(event[0]["result"], "Success")
            self.assertTrue(v["success"])
            self.assertEqual(event[0]["objects"], v["after"])
            after = objects(v["after"])
            targets = {key[1]: obj for key, obj in after.items() if key[0] == "target"}
            self.assertIn("Morphed {} objects.".format(len(targets)), v["history"])
            self.assertEqual(v["circle_command"], v["sdk_circle"])
            self.assertEqual(len(v["sdk_points"]), len(targets))
            self.assertEqual(len(v["sdk_ends"]), len(targets) if op["target_kind"] == "lines" else 0)
            for index, target in targets.items():
                # Native command and SDK outputs are bitwise equal in these captures.
                self.assertEqual(target["point"], v["sdk_points"][index], row["id"])
                compared += 1
                if op["target_kind"] == "lines":
                    self.assertEqual(target["end"], v["sdk_ends"][index], row["id"])
                    compared += 1
            for phase in ("before", "after", "undo", "redo"):
                for obj in v[phase]:
                    self.assertTrue(all(math.isfinite(c) for c in obj["point"]))
                    if "end" in obj:
                        self.assertTrue(all(math.isfinite(c) for c in obj["end"]))
        self.assertEqual(compared, 250)

    def test_selection_history_and_separate_definition_points(self):
        for op, row in zip(read("fixtures", "maelstrom_fit_points")["operations"],
                           read("observations", "maelstrom_fit_points")["results"]):
            v = row["value"]
            phases = {name: objects(v[name]) for name in ("before", "after", "undo", "redo")}
            self.assertEqual(objects(v["undo"], False), objects(v["before"], False), row["id"])
            self.assertEqual(objects(v["redo"], False), objects(v["after"], False), row["id"])
            for name, phase in phases.items():
                self.assertEqual(set(phase), set(phases["before"]))
                for key, obj in phase.items():
                    if op["target_kind"] == "points":
                        self.assertEqual(key[0], "target")
                        self.assertEqual(obj["selected"], name != "redo")
                    else:
                        selected = (name == "before" and key[0] == "target") or (name == "after" and key[0] == "definition")
                        self.assertEqual(obj["selected"], selected)
                    if key[0] == "definition":
                        self.assertEqual(obj["point"], op["points"][key[1]])
                        self.assertEqual(obj["kind"], "point")
            # Preselected point targets are accepted without a separate fitting selection.
            self.assertEqual("_SelID" in v["script_macro"], op["target_kind"] == "lines")

    def test_construction_plane_does_not_replace_the_fitted_frame(self):
        rows = read("observations", "maelstrom_fit_points")["results"]
        for index in (12, 13):
            for key in ("sdk_circle", "circle_command", "sdk_points", "sdk_ends"):
                self.assertEqual(rows[index]["value"][key], rows[3]["value"][key])

    def test_initial_driving_diagnostic_is_not_a_completed_morph(self):
        fixture = read("fixtures", "maelstrom_fit_points_driving_diagnostic")
        result = read("observations", "maelstrom_fit_points_driving_diagnostic")
        self.assertEqual(len(result["results"]), 14)
        self.assertEqual([op["id"] for op in fixture["operations"]], [row["id"] for row in result["results"]])
        for row in result["results"]:
            v = row["value"]
            self.assertTrue(v["success"])
            self.assertNotIn("Morphed ", v["history"])
            self.assertIn("Unknown command: _Copy=_No", v["history"])
            self.assertEqual(objects(v["before"], False), objects(v["after"], False))

    def test_provenance_hashes_include_initial_driving_diagnostic(self):
        record = json.loads((ROOT / "docs/maelstrom-fit-points-provenance.json").read_text())
        self.assertTrue(record["private_xvfb"])
        self.assertFalse(record["full_native_parity"])
        self.assertEqual(record["native_completed_commands"], 17)
        self.assertEqual(record["native_point_and_end_maps"], 250)
        self.assertEqual(record["initial_driving_diagnostics"], 14)
        for path, expected in record["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
