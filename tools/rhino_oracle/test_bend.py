"""Bounded Bend recipes, private launches and unmodified native evidence."""

import hashlib
import json
import math
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from . import bend_probe
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).parent
RECIPES = [
    ("bend_points", bend_probe.request),
    ("bend_command_points", bend_probe.command_request),
    ("bend_edge_points", bend_probe.edge_request),
    ("bend_command_followup", bend_probe.command_followup_request),
    ("bend_angle_points", bend_probe.angle_request),
    ("bend_angle_command", bend_probe.angle_command_request),
    ("bend_boundary_points", bend_probe.boundary_request),
    ("bend_small_angle_points", bend_probe.small_angle_request),
    ("bend_short_arc_points", bend_probe.short_arc_request),
    ("bend_validity_points", bend_probe.validity_request),
    ("bend_rigid_command", bend_probe.rigid_request),
]


class BendTests(unittest.TestCase):
    def test_factories_match_every_retained_input(self):
        for name, factory in RECIPES:
            request = factory()
            self.assertEqual(
                request, json.loads((ROOT / "fixtures" / (name + ".json")).read_text())
            )
            for op in request["operations"]:
                bend_probe.validate(op)

    def test_unbounded_values_and_command_injection_are_rejected(self):
        for factory in (bend_probe.request, bend_probe.command_request):
            original = factory()["operations"][0]
            for changes in [
                dict(id="unsafe\n_Delete"),
                dict(angle="90 _Delete"),
                dict(angle=True),
                dict(angle=float("inf")),
                dict(angle=float("nan")),
                dict(angle=5 * math.pi),
                dict(straight=1),
                dict(symmetric="Yes"),
                dict(start=[0, 0, 10]),
                dict(through=[1e7, 0, 0]),
                dict(through=[True, 0, 0]),
                dict(points=[]),
                dict(points=[[0, 0, 0]] * 257),
                dict(points=[[0, 0, float("nan")]]),
                dict(points=[[0, 0]]),
                dict(extra=True),
            ]:
                with self.subTest(changes=changes), self.assertRaises(ValueError):
                    bend_probe.validate(dict(original, **changes))
        original = bend_probe.command_request()["operations"][0]
        for key in ("copy", "rigid", "preserve", "non_attenuated", "grouped"):
            with self.subTest(key=key), self.assertRaises(ValueError):
                bend_probe.validate(dict(original, **{key: 1}))

    def test_private_scheme_and_single_iteration_are_required_before_launch(self):
        for scheme, iterations in [
            (None, 1),
            ("VibocerosOracleTest", True),
            ("VibocerosOracleTest", 2),
        ]:
            request = bend_probe.command_request()
            request["iterations"] = iterations
            with patch.dict(
                os.environ, {"DISPLAY": ":101", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
            ), patch(
                "tools.rhino_oracle.client._run_logged"
            ) as launch, self.assertRaises(
                OracleProtocolError
            ):
                OracleClient(launcher="/bin/true", settings_scheme=scheme).run_rhino(
                    request
                )
            launch.assert_not_called()

    def test_shared_desktop_display_cannot_launch_bend(self):
        with patch.dict(
            os.environ, {"DISPLAY": ":0", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
        ), patch(
            "tools.rhino_oracle.client._run_logged"
        ) as launch, self.assertRaisesRegex(
            OracleError, "dedicated Xvfb"
        ):
            OracleClient(launcher="/bin/true").run_rhino(bend_probe.request())
        launch.assert_not_called()

    def test_all_native_rows_have_matching_ids_validity_and_complete_samples(self):
        sdk = commands = successes = 0
        for name, factory in RECIPES:
            request = factory()
            captured = json.loads(
                (ROOT / "observations" / (name + ".json")).read_text()
            )
            self.assertEqual(captured["engine"], "rhino")
            self.assertEqual(captured["engine_version"], "8.32.26160.13001")
            self.assertEqual(len(request["operations"]), len(captured["results"]))
            for op, row in zip(request["operations"], captured["results"]):
                self.assertEqual(op["id"], row["id"])
                value = row["value"]
                if op["op"] == "bend_points":
                    sdk += 1
                    self.assertIs(type(value["valid"]), bool)
                    self.assertEqual(len(value["points"]), len(op["points"]))
                    if not value["valid"]:
                        self.assertEqual(value["points"], op["points"])
                else:
                    commands += 1
                    self.assertEqual(
                        [p["point"] for p in value["before"]], op["points"]
                    )
                    if not value["success"]:
                        self.assertEqual(value["before"], value["after"])
                        continue
                    successes += 1
                    event = next(e for e in value["events"] if e["name"] == "Bend")
                    self.assertEqual(event["result"], "Success")
                    count = len(op["points"])
                    self.assertEqual(
                        len(value["after"]), count * (2 if op["copy"] else 1)
                    )
                    self.assertEqual([p["point"] for p in value["undo"]], op["points"])
                    self.assertEqual(
                        [p["point"] for p in value["redo"]],
                        [p["point"] for p in value["after"]],
                    )
        self.assertEqual((sdk, commands, successes), (76, 48, 36))

    def test_failed_angle_syntax_is_retained_separately_from_corrected_captures(self):
        original = json.loads(
            (ROOT / "observations/bend_command_followup.json").read_text()
        )
        for row in original["results"][14:24]:
            self.assertFalse(row["value"]["success"])
            self.assertIn("_Angle=", row["value"]["history"])
        corrected = json.loads(
            (ROOT / "observations/bend_angle_command.json").read_text()
        )
        for i, row in enumerate(corrected["results"]):
            self.assertEqual(row["value"]["success"], i not in (4, 9))
            self.assertIn("_Angle\nBend Angle", row["value"]["history"])

    def test_provenance_hashes_keep_raw_evidence_reviewable(self):
        repo = ROOT.parents[1]
        provenance = json.loads((repo / "docs/bend-provenance.json").read_text())
        for path, digest in provenance["sha256"].items():
            self.assertEqual(
                hashlib.sha256((repo / path).read_bytes()).hexdigest(), digest, path
            )


if __name__ == "__main__":
    unittest.main()
