"""Owned Bend command recipes and retained, unmodified native terminal evidence."""

import copy
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from . import bend_command_probe as geometry, bend_options_probe as preferences
from . import twist_command_probe as twist
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).parent
RECIPES = [
    ("bend_geometry_command", geometry.request),
    ("bend_fitting_command", geometry.fitting_request),
    ("bend_limited_rigid_command", geometry.rigid_request),
    ("bend_rigid_midpoint_command", geometry.rigid_midpoint_request),
    ("bend_rigid_frame_boundary_command", geometry.rigid_boundary_request),
    ("bend_options_command", preferences.request),
    ("bend_options_followup_command", preferences.followup_request),
]


class BendCommandTests(unittest.TestCase):
    def test_shared_frame_boundary_has_independent_small_angle_twist_witnesses(self):
        request = dict(
            protocol_version=1,
            iterations=1,
            operations=[
                dict(
                    twist.request()["operations"][1],
                    id="twist-rigid-frame-boundary-" + str(i),
                    degrees=angle,
                )
                for i, angle in enumerate([-0.001, 1e-5, 0.001, 0.002, 0.004])
            ],
        )
        self.assertEqual(
            request,
            json.loads(
                (ROOT / "fixtures/twist_rigid_frame_boundary_command.json").read_text()
            ),
        )
        captured = json.loads(
            (ROOT / "observations/twist_rigid_frame_boundary_command.json").read_text()
        )
        self.assertEqual(len(captured["results"]), 5)
        for i, (op, row) in enumerate(zip(request["operations"], captured["results"])):
            twist.validate(op)
            self.assertEqual(op["id"], row["id"])
            self.assertTrue(row["value"]["success"])
            points = [
                o["geometry"]["points"][0] for o in row["value"]["after"]["objects"]
            ]
            span = max(
                max(p[k] for p in points) - min(p[k] for p in points) for k in (0, 1)
            )
            if i < 4:
                self.assertLess(span, 1e-7)
            else:
                self.assertGreater(span, 1e-7)

    def test_provenance_hashes_cover_inputs_raw_outputs_helpers_and_public_frame_checks(
        self,
    ):
        repo = ROOT.parents[1]
        provenance = json.loads(
            (repo / "docs/bend-command-provenance.json").read_text()
        )
        for path, digest in provenance["sha256"].items():
            self.assertEqual(
                hashlib.sha256((repo / path).read_bytes()).hexdigest(), digest, path
            )

    def test_every_retained_request_matches_its_bounded_factory(self):
        for name, factory in RECIPES:
            request = factory()
            self.assertEqual(
                request, json.loads((ROOT / "fixtures" / (name + ".json")).read_text())
            )
            probe = preferences if name.startswith("bend_options") else geometry
            for op in request["operations"]:
                probe.validate(op)

    def test_geometry_types_bounds_and_command_injection_are_rejected(self):
        original = geometry.request()["operations"][0]
        for key, values in (
            [
                ("id", [True, "unsafe\n_Delete"]),
                ("shape", ["Curve _Delete", None]),
                ("angle", [True, -1, 361, float("nan"), float("inf"), "90 _Delete"]),
                ("axis", [None, [], "Z _Delete"]),
                ("offset_z", [True, 101, float("nan")]),
                (
                    "through",
                    [[True, 0, 10], [101, 0, 10], [0, 10], [float("nan"), 0, 10]],
                ),
                ("tolerance", [True, 0, 1e-13, float("nan"), 0.1]),
                ("targets", [[], [[5, 0, 5]], [["0 _Delete", 0, 10]]]),
                ("extra", [True]),
            ]
            + [(key, [1, "Yes"]) for _, key in geometry.FLAGS]
            + [("grouped", [1])]
        ):
            for value in values:
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    geometry.validate(dict(original, **{key: value}))
        geometry.validate(dict(original, copy=True, targets=[[5, 0, 5]]))

    def test_preference_workflows_reject_unbounded_commands_and_bad_values(self):
        original = preferences.request()["operations"][0]
        for steps in [
            [],
            [dict(kind="Delete")],
            [dict(kind="New", script="_Delete")],
            [dict(kind="RememberCopyOptions", enabled=1)],
        ]:
            with self.subTest(steps=steps), self.assertRaises(ValueError):
                preferences.validate(dict(original, steps=steps))
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
                preferences.validate(
                    dict(original, steps=[dict(original["steps"][0], **changes)])
                )

    def test_private_scheme_and_iteration_checks_precede_any_launch(self):
        for factory in (geometry.request, preferences.request):
            for scheme, iterations in [
                (None, 1),
                ("VibocerosOracleTest", True),
                ("VibocerosOracleTest", 2),
            ]:
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

    def test_shared_desktop_display_cannot_launch_bend_commands(self):
        for factory in (geometry.request, preferences.request):
            with patch.dict(
                os.environ, {"DISPLAY": ":0", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
            ), patch(
                "tools.rhino_oracle.client._run_logged"
            ) as launch, self.assertRaisesRegex(
                OracleError, "dedicated Xvfb"
            ):
                OracleClient(
                    launcher="/bin/true", settings_scheme="VibocerosOracleTest"
                ).run_rhino(factory())
            launch.assert_not_called()

    def test_geometry_captures_include_attributes_groups_and_complete_history(self):
        def geometry_state(state):
            state = copy.deepcopy(state)
            for obj in state["objects"]:
                del obj["selected"]
            return state

        count = 0
        for name, factory in RECIPES:
            if name.startswith("bend_options"):
                continue
            request = factory()
            captured = json.loads(
                (ROOT / "observations" / (name + ".json")).read_text()
            )
            self.assertEqual(captured["engine_version"], "8.32.26160.13001")
            self.assertEqual(len(request["operations"]), len(captured["results"]))
            for op, row in zip(request["operations"], captured["results"]):
                count += 1
                self.assertEqual(op["id"], row["id"])
                value = row["value"]
                self.assertTrue(value["success"])
                self.assertEqual(
                    next(e["result"] for e in value["events"] if e["name"] == "Bend"),
                    "Success",
                )
                before = geometry_state(value["before"])
                undo = geometry_state(value["undo"])
                self.assertEqual(before["objects"], undo["objects"])
                # Rhino retains newly allocated Copy group definitions after
                # Undo, with empty membership. The original groups are intact.
                self.assertEqual(
                    before["groups"], undo["groups"][: len(before["groups"])]
                )
                self.assertTrue(
                    all(
                        not g["members"]
                        for g in undo["groups"][len(before["groups"]) :]
                    )
                )
                self.assertEqual(
                    geometry_state(value["after"]), geometry_state(value["redo"])
                )
        self.assertEqual(count, 76)

    def test_native_fitting_definitions_stop_refining_below_one_e_minus_five(self):
        rows = json.loads(
            (ROOT / "observations/bend_fitting_command.json").read_text()
        )["results"]
        for offset in (0, 3, 6, 9):
            low = rows[offset + 1]["value"]["after"]["objects"][0]["geometry"]
            tight = rows[offset + 2]["value"]["after"]["objects"][0]["geometry"]
            for key in ("definition", "surfaces"):
                if key in low:
                    self.assertEqual(low[key], tight[key])

    def test_native_canceled_options_have_two_distinct_remembering_policies(self):
        records = json.loads(
            (ROOT / "observations/bend_options_command.json").read_text()
        )["results"][0]["value"]["records"]
        self.assertEqual(len(records), 27)
        for index, name in [(1, "Rigid"), (7, "PreserveStructure")]:
            row = records[index]
            self.assertEqual(
                row["query_after"]["defaults"][name],
                row["query_before"]["defaults"][name],
            )
        for index, name in [
            (3, "LimitToSpine"),
            (5, "Symmetric"),
            (9, "NonAttenuated"),
        ]:
            self.assertTrue(records[index]["query_after"]["defaults"][name])
        self.assertEqual(records[14]["query_after"]["angle_default"], 90.0)
        self.assertIsNone(records[15]["query_after"]["angle_default"])
        self.assertIsNone(records[22]["query_after"]["angle_default"])
        for index in (23, 24, 25):
            self.assertEqual(
                records[index]["query_after"]["defaults"],
                records[22]["query_after"]["defaults"],
            )


if __name__ == "__main__":
    unittest.main()
