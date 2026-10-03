"""Bounded public Twist captures, private launches and retained native evidence."""

import copy
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch
from .client import OracleClient, OracleProtocolError
from . import twist_probe, twist_command_probe

ROOT = Path(__file__).parent


class TwistTests(unittest.TestCase):
    def test_factories_match_all_saved_inputs(self):
        for filename, factory in [
            ("twist_points", twist_probe.request),
            ("twist_command", twist_command_probe.request),
            ("twist_rigid_command", twist_command_probe.rigid_request),
            ("twist_repeat_command", twist_command_probe.repeat_request),
        ]:
            self.assertEqual(
                json.loads((ROOT / "fixtures" / (filename + ".json")).read_text()),
                factory(),
            )
            for op in factory()["operations"]:
                (
                    twist_probe if filename == "twist_points" else twist_command_probe
                ).validate(op)

    def test_command_types_presets_bounds_and_injection_are_rejected(self):
        original = twist_command_probe.request()["operations"][0]
        for key, values in [
            ("shape", ["Delete", None, 2]),
            ("copy", [1, "Yes"]),
            ("id", ["unsafe path", "x\n_Delete", True]),
            ("degrees", [float("nan"), float("inf"), True, 1441]),
            ("axis", ["Z _Delete", None]),
            ("offset_z", [101, float("nan"), True]),
            ("tolerance", [0, True, float("nan")]),
            ("angles", [[], [90], ["90 _Delete"]]),
            ("extra", [True]),
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
            request = twist_command_probe.request()
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


if __name__ == "__main__":
    unittest.main()
