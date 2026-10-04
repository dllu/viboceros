"""Retained native grip inputs, replay state, and private launch boundaries."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .circle_fit_grips_probe import request, validate
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


def read(folder):
    return json.loads((ROOT / "tools/rhino_oracle" / folder / "circle_fit_grips.json").read_text())


class CircleGripTests(TestCase):
    def test_closed_inputs_and_private_launch_guards(self):
        q = request()
        self.assertEqual(q, read("fixtures"))
        for change in (dict(extra=True), dict(id="x\n_Delete"), dict(source="anything"),
                       dict(controls=[[True, 0, 0]]*4), dict(controls=[[float("nan"), 0, 0]]*4),
                       dict(controls=[[101, 0, 0]]*4), dict(controls=[[0, 0, 0]]*65),
                       dict(weights=[True]*4), dict(weights=[float("inf")]*4), dict(weights=[1]),
                       dict(selected=[True]), dict(selected=[0, 0]), dict(selected=[4]), dict(selected=[]),
                       dict(points=[[False, 0, 0]]), dict(inputs="all _Exit"), dict(off=1)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(q["operations"][0], **change))
        for scheme, count, display, headless in ((None, 1, ":301", ":301"),
                ("VibocerosOracleGrips", True, ":301", ":301"),
                ("VibocerosOracleGrips", 2, ":301", ":301"),
                ("VibocerosOracleGrips", 1, ":0", None),
                ("VibocerosOracleGrips", 1, ":301", ":302")):
            data = request()
            data["iterations"] = count
            env = {"DISPLAY": display}
            if headless:
                env["VIBOCEROS_ORACLE_HEADLESS"] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(data, 1)
                launch.assert_not_called()

    def test_native_grip_count_weighting_cancellation_and_undo(self):
        q, r = read("fixtures"), read("observations")
        self.assertEqual(r["engine_version"], "8.32.26160.13001")
        self.assertEqual(len(r["results"]), 16)
        self.assertEqual([op["id"] for op in q["operations"]], [row["id"] for row in r["results"]])
        for i, (op, row) in enumerate(zip(q["operations"], r["results"])):
            with self.subTest(id=op["id"]):
                v = row["value"]
                # EndCommand precedes getter cleanup. The final script state
                # isolates the following Undo's effects without idle Escape.
                final = v["after_script"]
                expected_final = v["after"]
                if op["inputs"] == "all":
                    expected_final = [dict(o, selected=False, grips=[dict(g, selected=False) for g in o["grips"]])
                                      if o["kind"] == op["source"] else dict(o, selected=False) for o in v["after"]]
                self.assertEqual(final, expected_final)
                before, after = v["before"][0], v["after"][0]
                self.assertTrue(before["grips_on"])
                self.assertEqual(before["selected"], 12 <= i <= 14)
                self.assertEqual([g["index"] for g in before["grips"]], list(range(len(op["controls"]))))
                self.assertEqual([g["point"] for g in before["grips"]], op["controls"])
                self.assertEqual([g["index"] for g in before["grips"] if g["selected"]], sorted(op["selected"]))
                self.assertEqual(v["success"], i != 3)
                self.assertEqual(after.get("definition"), before.get("definition"))
                self.assertEqual(after.get("vertices"), before.get("vertices"))
                circles = [o for o in v["after"] if o["kind"] == "circle"]
                self.assertEqual(len(circles), int(i not in (3, 10)))
                if circles:
                    self.assertFalse(circles[0]["selected"])
                    expected_radius = 8**.5 if op["source"] == "surface" else 2.
                    self.assertAlmostEqual(circles[0]["circle"]["radius"], expected_radius, places=13)
                    self.assertEqual(v["undo"][0]["grips_on"], not op["off"])
                    self.assertEqual(v["undo"][0]["grips"], [] if op["off"] else final[0]["grips"])
                    self.assertEqual(v["redo"][0], v["undo"][0])
                    for actual, expected in zip(v["undo"], final):
                        self.assertEqual(actual["selected"], expected["selected"])
                else:
                    self.assertEqual(v["undo"], [])
                    self.assertEqual(v["redo"], final)
                if i == 3:
                    self.assertTrue(all(not g["selected"] for g in after["grips"]))
                if op["off"]:
                    self.assertFalse(v["points_off"][0]["grips_on"])
                    self.assertEqual(v["points_off"][0]["grips"], [])
                    self.assertEqual([o["kind"] for o in v["points_off"]], [o["kind"] for o in v["after"]])
                    self.assertEqual([o["selected"] for o in v["points_off"]], [o["selected"] for o in final])
        # Coincident mesh vertices remain distinct grips and fit inputs.
        mesh = r["results"][11]["value"]["after"][0]["grips"]
        self.assertEqual(len(mesh), 5)
        self.assertEqual(mesh[0]["point"], mesh[4]["point"])
        self.assertTrue(mesh[0]["selected"] and mesh[4]["selected"])

    def test_provenance_hashes(self):
        record = json.loads((ROOT / "docs/circle-fit-grips-provenance.json").read_text())
        self.assertTrue(record["private_xvfb"])
        self.assertFalse(record["full_native_parity"])
        self.assertEqual(record["native_recipes"], 16)
        for path, expected in record["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
