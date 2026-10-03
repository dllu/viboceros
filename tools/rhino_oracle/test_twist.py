"""Bounded public Twist captures, private launches and retained native evidence."""

import copy
import json
import math
import os
from pathlib import Path
import unittest
from unittest.mock import patch
from .client import OracleClient, OracleProtocolError
from . import twist_probe, twist_command_probe, twist_options_probe

ROOT = Path(__file__).parent


class TwistTests(unittest.TestCase):
    def test_factories_match_all_saved_inputs(self):
        for filename, factory in [
            ("twist_points", twist_probe.request),
            ("twist_command", twist_command_probe.request),
            ("twist_rigid_command", twist_command_probe.rigid_request),
            ("twist_repeat_command", twist_command_probe.repeat_request),
            ("twist_tight_command", twist_command_probe.tight_request),
            ("twist_options_command", twist_options_probe.request),
            ("twist_fitting_command", twist_command_probe.fitting_request),
        ]:
            self.assertEqual(
                json.loads((ROOT / "fixtures" / (filename + ".json")).read_text()),
                factory(),
            )
            for op in factory()["operations"]:
                probe = (
                    twist_probe
                    if filename == "twist_points"
                    else (
                        twist_options_probe
                        if filename == "twist_options_command"
                        else twist_command_probe
                    )
                )
                probe.validate(op)

    def test_command_types_presets_bounds_and_injection_are_rejected(self):
        original = twist_command_probe.request()["operations"][0]
        for key, values in [
            ("shape", ["Delete", None, 2]),
            ("copy", [1, "Yes"]),
            ("id", ["unsafe path", "x\n_Delete", True]),
            ("degrees", [float("nan"), float("inf"), True, 1441]),
            ("axis", ["Z _Delete", None]),
            ("offset_z", [101, float("nan"), True]),
            ("tolerance", [0, 1e-13, 0.1, True, float("nan")]),
            ("angles", [[], [90], ["90 _Delete"]]),
            ("extra", [True]),
            ("sdk_fit", [1, "Yes"]),
        ]:
            for value in values:
                op = dict(original, **{key: value})
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    twist_command_probe.validate(op)
        op = dict(original, copy=True, angles=[90])
        twist_command_probe.validate(op)

    def test_point_types_axes_and_bounds_are_rejected(self):
        original = twist_probe.request()["operations"][0]
        for changes in [
            dict(angle=True),
            dict(angle=float("nan")),
            dict(points=[]),
            dict(points=[[1, 2, 3]] * 257),
            dict(points=[[1, float("inf"), 2]]),
            dict(axis_end=[0, 0, 0]),
            dict(extra=True),
            dict(infinite=1),
        ]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                twist_probe.validate(dict(original, **changes))

    def test_private_scheme_and_iteration_checks_precede_launch(self):
        for scheme, iterations in [
            (None, 1),
            ("VibocerosOracleTest", True),
            ("VibocerosOracleTest", 2),
        ]:
            for factory in (twist_command_probe.request, twist_options_probe.request):
                request = factory()
                request["iterations"] = iterations
                with patch.dict(
                    os.environ, {"DISPLAY": ":101", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
                ), patch(
                    "tools.rhino_oracle.client._run_logged"
                ) as launch, self.assertRaises(
                    OracleProtocolError
                ):
                    OracleClient(
                        launcher="/bin/true", settings_scheme=scheme
                    ).run_rhino(request)
                launch.assert_not_called()

    def test_preference_workflows_reject_unbounded_commands_and_bad_values(self):
        original = twist_options_probe.request()["operations"][0]
        for changes in [
            dict(extra=True),
            dict(steps=[]),
            dict(steps=[dict(kind="Delete")]),
            dict(steps=[dict(kind="New", script="_Delete")]),
            dict(steps=[dict(kind="RememberCopyOptions", enabled=1)]),
        ]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                twist_options_probe.validate(dict(original, **changes))
        first = original["steps"][0]
        for changes in [
            dict(shape="Curve _Delete"),
            dict(finish="Complete _Delete"),
            dict(options={"Rigid": 1}),
            dict(options={"Unknown": True}),
            dict(degrees=True),
            dict(degrees=float("inf")),
            dict(shape="Box", options={"PreserveStructure": True}),
        ]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                twist_options_probe.validate(
                    dict(original, steps=[dict(first, **changes)])
                )

    def test_desktop_display_cannot_launch_twist(self):
        with patch.dict(
            os.environ, {"DISPLAY": ":0", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
        ), patch(
            "tools.rhino_oracle.client._run_logged"
        ) as launch, self.assertRaisesRegex(
            Exception, "dedicated Xvfb"
        ):
            OracleClient(
                launcher="/bin/true", settings_scheme="VibocerosOracleTest"
            ).run_rhino(twist_command_probe.request())
        launch.assert_not_called()

    def test_raw_native_terminal_snapshots_selection_colors_and_repetition(self):
        for filename, expected_count in [
            ("twist_command", 36),
            ("twist_rigid_command", 12),
            ("twist_repeat_command", 4),
        ]:
            data = json.loads(
                (ROOT / "observations" / (filename + ".json")).read_text()
            )
            self.assertEqual(data["engine"], "rhino")
            self.assertEqual(len(data["results"]), expected_count)
            for row in data["results"]:
                v = row["value"]
                self.assertTrue(v["success"])
                event = next(e for e in v["events"] if e["name"] == "Twist")
                self.assertEqual(event["result"], "Success")
                self.assertEqual(event["objects"], v["after"])
                if v["undo"] is not None:
                    self.assertEqual(
                        len(v["undo"]["objects"]), len(v["before"]["objects"])
                    )
                for obj in v["after"]["objects"]:
                    if obj["geometry"]["type"] == "Mesh":
                        self.assertEqual(
                            obj["geometry"]["colors"],
                            [[20 + i, 40 + i, 60 + i, 255] for i in range(4)],
                        )
                if filename == "twist_repeat_command":
                    self.assertEqual(len(v["after"]["objects"]), 21)

    def test_native_fitting_floor_is_observed_in_commands_and_public_sdk(self):
        request = twist_command_probe.fitting_request()
        data = json.loads(
            (ROOT / "observations/twist_fitting_command.json").read_text()
        )
        self.assertEqual(len(data["results"]), 18)
        for op, row in zip(request["operations"], data["results"]):
            self.assertEqual(op["id"], row["id"])
            value = row["value"]
            self.assertTrue(value["success"])
            self.assertEqual(value["sdk_fit"]["tolerance"], op["tolerance"])
            self.assertTrue(value["sdk_fit"]["results"][0]["success"])
            if op["shape"] != "Box":
                self.assertEqual(
                    value["after"]["objects"][0]["geometry"],
                    value["sdk_fit"]["results"][0]["geometry"],
                )
        for shape in ("Line", "Curve", "Surface", "Box"):
            cases = [
                (op, row["value"]["after"]["objects"][0]["geometry"])
                for op, row in zip(request["operations"], data["results"])
                if op["shape"] == shape
            ]
            tight = cases[-1][1]
            for op, geometry in cases:
                if op["tolerance"] <= 1e-5:
                    self.assertEqual(geometry, tight)
                else:
                    self.assertNotEqual(geometry, tight)
        tight = json.loads((ROOT / "observations/twist_tight_command.json").read_text())
        self.assertEqual(len(tight["results"]), 12)
        for row in tight["results"]:
            value = row["value"]
            self.assertTrue(value["success"])
            event = next(e for e in value["events"] if e["name"] == "Twist")
            self.assertEqual(event["result"], "Success")
            self.assertEqual(event["objects"], value["after"])
        # Native fitted samples themselves exceed the tiny document tolerance;
        # matching them must use the measured fitting policy as its error bound.
        for index in (0, 5, 8):
            value = tight["results"][index]["value"]
            source = value["before"]["objects"][0]["geometry"]["samples"]
            output = value["after"]["objects"][0]["geometry"]["samples"]
            errors = []
            for p, q in zip(source, output):
                t = p[2] / 10 if index == 8 else max(0, min(1, p[2] / 10))
                angle = math.pi / 2 * (t if index == 8 else t * t * (3 - 2 * t))
                s, c = math.sin(angle), math.cos(angle)
                target = [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
                errors.append(math.sqrt(sum((a - b) ** 2 for a, b in zip(q, target))))
            self.assertGreater(max(errors), 1e-7)
            self.assertLess(max(errors), 1e-5)

    def test_native_preference_workflow_retains_terminal_events_and_cancelled_edits(
        self,
    ):
        request = twist_options_probe.request()["operations"][0]
        data = json.loads(
            (ROOT / "observations/twist_options_command.json").read_text()
        )
        records = data["results"][0]["value"]["records"]
        self.assertEqual(len(records), 39)
        self.assertEqual([r["step"] for r in records], request["steps"])
        for record in records:
            for query in (record["query_before"], record["query_after"]):
                self.assertFalse(query["success"])
                self.assertEqual(
                    set(query["defaults"]), set(twist_options_probe.OPTIONS)
                )
                event = next(e for e in query["events"] if e["name"] == "Twist")
                self.assertEqual(event["result"], "Cancel")
                self.assertEqual(event["objects"], query["after"])
            step = record["step"]
            if step["kind"] != "Twist":
                continue
            result = record["result"]
            event = next(e for e in result["events"] if e["name"] == "Twist")
            self.assertEqual(event["objects"], result["after"])
            if step["finish"] in ("Cancel", "ReferenceCancel"):
                self.assertEqual(event["result"], "Cancel")
                self.assertEqual(result["after"], record["before"])
                self.assertEqual(
                    record["query_before"]["defaults"],
                    record["query_after"]["defaults"],
                )
            elif step["finish"] == "CopyThenCancel":
                self.assertFalse(result["success"])
                self.assertEqual(event["result"], "Cancel")
                self.assertEqual(len(result["after"]), len(record["before"]) + 7)
                self.assertEqual(record["query_after"]["defaults"], step["options"])


if __name__ == "__main__":
    unittest.main()
