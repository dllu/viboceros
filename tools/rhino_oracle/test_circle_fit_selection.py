"""Native Circle FitPoints selection and cancellation, without frame claims."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .circle_fit_selection_probe import request, validate
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


def read(folder):
    return json.loads((ROOT / "tools/rhino_oracle" / folder / "circle_fit_selection.json").read_text())


class CircleFitSelectionTests(TestCase):
    def test_closed_recipes_ids_and_private_launch_guards(self):
        q = request()
        self.assertEqual(q, read("fixtures"))
        r = read("observations")
        self.assertEqual(r["engine_version"], "8.32.26160.13001")
        self.assertEqual([op["id"] for op in q["operations"]], [row["id"] for row in r["results"]])
        self.assertEqual(len(r["results"]), 8)
        for changes in (dict(extra=True), dict(id="x\n_Delete"), dict(points=[[True, 0, 0]]),
                        dict(points=[[float("nan"), 0, 0]]), dict(points=[[101, 0, 0]]),
                        dict(points=[[0, 0, 0]]*65), dict(preselected=[True]),
                        dict(preselected=[0, 0]), dict(preselected=[3]), dict(preselected=[]), dict(line=1),
                        dict(inputs="all _Exit")):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(q["operations"][0], **changes))
        for scheme, count, display, headless in ((None, 1, ":301", ":301"),
                ("VibocerosOracleCircleSelection", True, ":301", ":301"),
                ("VibocerosOracleCircleSelection", 2, ":301", ":301"),
                ("VibocerosOracleCircleSelection", 1, ":0", None),
                ("VibocerosOracleCircleSelection", 1, ":301", ":302")):
            data = request()
            data["iterations"] = count
            env = {"DISPLAY": display}
            if headless:
                env["VIBOCEROS_ORACLE_HEADLESS"] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(data, 1)
                launch.assert_not_called()

    def test_native_minimum_count_preselection_cancellation_and_history(self):
        results = read("observations")["results"]
        self.assertEqual([r["value"]["success"] for r in results], [True, True, False, False, True, False, True, True])
        for i, (op, row) in enumerate(zip(read("fixtures")["operations"], results)):
            v = row["value"]
            events = [event for event in v["events"] if event["name"] == "Circle"]
            self.assertEqual(len(events), 1)
            self.assertEqual(v["success"], events[0]["result"] == "Success")
            self.assertEqual(events[0]["objects"], v["after"])
            final = v["after_script"]
            circles = [o for o in v["after"] if o["kind"] == "circle"]
            self.assertEqual(len(circles), int(i in (0, 1, 4, 6)))
            for phase in ("before", "after", "redo"):
                self.assertEqual([o["point"] for o in v[phase] if o["kind"] == "point"], op["points"])
            if circles:
                self.assertFalse(circles[0]["selected"])
                self.assertAlmostEqual(circles[0]["circle"]["radius"], 2., places=13)
                expected = v["before"] if op["inputs"] == "auto" else [dict(o, selected=False) for o in v["before"]]
                self.assertEqual(v["undo"], expected)
                self.assertEqual(v["redo"], final)
                self.assertTrue(all(o["selected"] for o in v["after"] if o["kind"] == "point"))
            else:
                self.assertEqual(v["undo"], [])
                self.assertEqual(v["redo"], final)
            expected_final = [dict(o, selected=False) for o in v["after"]] if op["inputs"] == "all" else v["after"]
            self.assertEqual(final, expected_final)
            if not v["success"]:
                self.assertTrue(all(not o["selected"] for o in v["after"]))
            if i in (0, 4, 7):
                self.assertNotIn("Select points to build a circle from", v["history"])
            else:
                self.assertIn("Select points to build a circle from", v["history"])
        # Unsupported preselection survives automatic acceptance, but is cleared
        # when the command needs its separate point selection getter.
        self.assertTrue(next(o for o in results[4]["value"]["after"] if o["kind"] == "line")["selected"])
        self.assertFalse(next(o for o in results[6]["value"]["after"] if o["kind"] == "line")["selected"])

    def test_provenance_hashes(self):
        record = json.loads((ROOT / "docs/circle-fit-input-provenance.json").read_text())
        self.assertTrue(record["private_xvfb"])
        self.assertFalse(record["full_native_parity"])
        self.assertEqual(record["native_selection_recipes"], 8)
        for path, expected in record["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
