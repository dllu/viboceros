"""Open-volume diagnostic input is explicit, bounded and ownership-checked."""
import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from .area_centroid_probe import validate
from .area_centroid_replay import prepare
from .client import OracleError, OracleProtocolError
from .references.volume_centroid_confirmation import request
from .volume_confirmation import DIALOG_TITLE, KEYS, VolumeConfirmation, _owned_dialog

MODULE = "tools.rhino_oracle.volume_confirmation."
DIALOG = dict(window="0x10", owner_pid=42, parent_window="0x20", title=DIALOG_TITLE)
ROOT = Path(__file__).resolve().parents[2]


class VolumeConfirmationTests(unittest.TestCase):
    def setUp(self):
        self.request = request()
        self.request["operations"] = self.request["operations"][:1]
        self.name = self.request["operations"][0]["id"]
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.job = Path(self.directory.name)
        self.progress = self.job / "worker-progress.log"

    def marker(self, choice="yes"):
        self.progress.write_text("VOLUME_CONFIRM %s %s\n" % (self.name, choice))

    def test_requires_explicit_valid_volume_only_choices_and_unique_ids(self):
        for change in [dict(open_confirmation=True), dict(open_confirmation="Yes"), dict(open_confirmation="cancel"),
                       dict(open_confirmation="yes\ncommand"), dict(open_confirmation=None),
                       dict(op="area_centroid_command"), dict(id="bad\nmarker")]:
            invalid = copy.deepcopy(self.request)
            invalid["operations"][0].update(change)
            with self.subTest(change=change), self.assertRaises(OracleProtocolError):
                VolumeConfirmation(invalid)
        for invalid in [dict(self.request, operations=self.request["operations"]*2),
                        dict(self.request, iterations=True), dict(self.request, iterations=2),
                        dict(self.request, protocol_version=True), dict(self.request, operations=[])]:
            with self.assertRaises(OracleProtocolError): VolumeConfirmation(invalid)
        area = dict(self.request["operations"][0], op="area_centroid_command")
        with self.assertRaises(ValueError): validate(area)
        with self.assertRaisesRegex(OracleProtocolError, "not yet supported"):
            prepare(self.request, {}, measure="volume")

    def test_no_marker_no_owned_pid_or_no_matching_dialog_means_no_input(self):
        responder = VolumeConfirmation(self.request)
        with patch(MODULE+"_owned_dialog", return_value=None) as dialog, patch(MODULE+"subprocess.run") as run:
            responder(self.job, {42})
            self.marker()
            responder(self.job, set())
            dialog.assert_not_called()
            responder(self.job, {42})
            dialog.assert_called_once_with({42})
            run.assert_not_called()

    def test_each_choice_is_delivered_once_to_the_verified_dialog(self):
        for choice, key in KEYS.items():
            self.request["operations"][0]["open_confirmation"] = choice
            self.marker(choice)
            responder = VolumeConfirmation(self.request)
            responder.ready[self.name,"0x10"] = 0
            with patch(MODULE+"_owned_dialog", return_value=DIALOG), patch(MODULE+"subprocess.run") as run:
                responder(self.job, {42})
                responder(self.job, {42})
                run.assert_called_once_with(["xdotool", "windowactivate", "--sync", "0x10", "key",
                                             "--clearmodifiers", key],
                                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
            self.assertEqual(responder.delivered, {self.name:DIALOG})

    def test_completed_or_racing_marker_cannot_receive_input(self):
        responder = VolumeConfirmation(self.request)
        responder.ready[self.name,"0x10"] = 0
        self.marker()
        self.progress.write_text(self.progress.read_text()+"VOLUME_CONFIRM_DONE %s\n" % self.name)
        with patch(MODULE+"_owned_dialog") as dialog, patch(MODULE+"subprocess.run") as run:
            responder(self.job, {42})
            dialog.assert_not_called()
            run.assert_not_called()
        self.marker()
        def completed(_):
            self.progress.write_text(self.progress.read_text()+"VOLUME_CONFIRM_DONE %s\n" % self.name)
            return DIALOG
        with patch(MODULE+"_owned_dialog", side_effect=completed), patch(MODULE+"subprocess.run") as run:
            responder(self.job, {42})
            run.assert_not_called()

    def test_settling_is_nonblocking_and_rechecks_ownership_before_delivery(self):
        self.marker()
        responder = VolumeConfirmation(self.request)
        with patch(MODULE+"_owned_dialog", side_effect=[DIALOG, None, DIALOG]) as dialog, patch(
                MODULE+"subprocess.run") as run:
            with patch(MODULE+"time.monotonic", return_value=10.):
                responder(self.job, {42})
            run.assert_not_called()
            with patch(MODULE+"time.monotonic", return_value=11.):
                responder(self.job, {42})
                run.assert_not_called()
                responder(self.job, {42})
            self.assertEqual(dialog.call_count, 3)
            self.assertEqual(run.call_count, 1)

    def test_unknown_wrong_choice_and_duplicate_pending_markers_fail_closed(self):
        for marker in ["VOLUME_CONFIRM other yes\n", "VOLUME_CONFIRM %s no\n" % self.name,
                       ("VOLUME_CONFIRM %s yes\n" % self.name)*2]:
            self.progress.write_text(marker)
            with patch(MODULE+"_owned_dialog") as dialog, self.assertRaises(OracleError):
                VolumeConfirmation(self.request)(self.job, {42})
            dialog.assert_not_called()

    def test_input_failure_is_not_recorded_as_delivery(self):
        self.marker()
        responder = VolumeConfirmation(self.request)
        responder.ready[self.name,"0x10"] = 0
        with patch(MODULE+"_owned_dialog", return_value=DIALOG), patch(MODULE+"subprocess.run", side_effect=OSError("input failed")):
            with self.assertRaises(OracleError): responder(self.job, {42})
        self.assertEqual(responder.delivered, {})

    def test_diagnostics_distinguish_requested_choice_from_actual_delivery(self):
        responder = VolumeConfirmation(self.request)
        response = dict(results=[dict(id=self.name, value={})])
        responder.record_diagnostics(response)
        self.assertEqual(response["results"][0]["value"]["open_confirmation"], dict(requested="yes", dialog=None))
        responder.delivered[self.name] = DIALOG
        responder.record_diagnostics(response)
        self.assertEqual(response["results"][0]["value"]["open_confirmation"]["dialog"], DIALOG)
        with self.assertRaises(OracleProtocolError): responder.record_diagnostics(dict(results=[]))

    def test_window_pid_title_and_parent_must_all_match(self):
        listing = "0x10 0 42 host %s\n" % DIALOG_TITLE
        good = [(42,"0x20",DIALOG_TITLE), (42,None,"Untitled - Rhino 8 Educational")]
        invalid = [[(43,"0x20",DIALOG_TITLE)], [(42,None,DIALOG_TITLE)], [(42,"0x20","Unrelated warning")],
                   [good[0],(43,None,"Untitled - Rhino 8 Educational")], [good[0],(42,None,"Other app")],
                   [good[0],(42,None,DIALOG_TITLE)], [(42,"0x10",DIALOG_TITLE),good[1]]]
        with patch(MODULE+"_query", return_value=listing), patch(MODULE+"_properties", side_effect=good):
            self.assertEqual(_owned_dialog({42}), DIALOG)
        for properties in invalid:
            with patch(MODULE+"_query", return_value=listing), patch(MODULE+"_properties", side_effect=properties):
                self.assertIsNone(_owned_dialog({42}))
        for windows in [listing.replace("42 host", "43 host"), listing.replace(DIALOG_TITLE,"Another dialog"),
                        "garbage", listing.replace("0x10", "bad-window")]:
            with patch(MODULE+"_query", return_value=windows), patch(MODULE+"_properties") as properties:
                self.assertIsNone(_owned_dialog({42}))
                properties.assert_not_called()
        with patch(MODULE+"_query") as query:
            self.assertIsNone(_owned_dialog(set()))
            query.assert_not_called()

    def test_multiple_owned_dialogs_are_not_guessed(self):
        listing = "0x10 0 42 host %s\n0x11 0 42 host %s\n" % (DIALOG_TITLE,DIALOG_TITLE)
        properties = [(42,"0x20",DIALOG_TITLE), (42,None,"Untitled - Rhino 8 Educational")]*2
        with patch(MODULE+"_query", return_value=listing), patch(MODULE+"_properties", side_effect=properties):
            with self.assertRaisesRegex(OracleError,"ambiguous"): _owned_dialog({42})

    def test_real_property_format_is_parsed_and_failed_queries_do_not_match(self):
        listing = "0x10 0 42 host %s\n" % DIALOG_TITLE
        props = '_NET_WM_PID(CARDINAL) = 42\nWM_TRANSIENT_FOR(WINDOW): window id # 0x20\nWM_NAME(STRING) = "%s"\n' % DIALOG_TITLE
        parent = '_NET_WM_PID(CARDINAL) = 42\nWM_TRANSIENT_FOR: not found.\nWM_NAME(STRING) = "Untitled - Rhino 8 Educational"\n'
        def result(output, code=0):
            return subprocess.CompletedProcess([], code, stdout=output)
        with patch(MODULE+"subprocess.run", side_effect=[result(listing),result(props),result(parent)]):
            self.assertEqual(_owned_dialog({42}), DIALOG)
        with patch(MODULE+"subprocess.run", return_value=result(listing,1)):
            self.assertIsNone(_owned_dialog({42}))
        with patch(MODULE+"subprocess.run", side_effect=subprocess.TimeoutExpired("wmctrl",5)):
            with self.assertRaises(OracleError): _owned_dialog({42})

    def test_retained_sources_and_full_geometry_evidence_are_independent_of_targets(self):
        data = json.loads((ROOT/"tools/rhino_oracle/fixtures/volume_centroid_confirmation.json").read_text())
        observed = json.loads((ROOT/"tools/rhino_oracle/observations/volume_centroid_confirmation.json").read_text())
        self.assertEqual(data, request())
        self.assertEqual(len(data["operations"]), 42)
        self.assertEqual([op["id"] for op in data["operations"]], [row["id"] for row in observed["results"]])
        with self.assertRaisesRegex(OracleProtocolError, "not yet supported"):
            prepare(data, observed, measure="volume")
        # Reuse the existing full source/geometry/diagnostic validator, not the
        # native runner. This validates capture fidelity, never compatibility.
        for operation, row in zip(data["operations"], observed["results"]):
            self.assertEqual(row["value"]["open_confirmation"]["requested"], operation.pop("open_confirmation"))
            row["value"].pop("open_confirmation")
        self.assertEqual(prepare(data, observed, measure="volume")[0], data)
        for row in observed["results"]:
            for point in row["value"]["points"]: point["point"] = [123.,456.,789.]
        self.assertEqual(prepare(data, observed, measure="volume")[0], data)

    def test_retained_warning_choices_selection_and_api_differences_are_not_normalized(self):
        data = request()
        observed = json.loads((ROOT/"tools/rhino_oracle/observations/volume_centroid_confirmation.json").read_text())
        warned, successful = 0, 0
        for operation, row in zip(data["operations"], observed["results"]):
            value = row["value"]
            closed = operation["id"].startswith(("closed-tetra-", "unoriented-tetra-"))
            dialog = value["open_confirmation"]["dialog"]
            self.assertEqual(dialog is None, closed)
            if dialog is not None:
                warned += 1
                self.assertEqual(dialog["title"], DIALOG_TITLE)
                self.assertIs(type(dialog["owner_pid"]), int)
                self.assertGreater(dialog["owner_pid"], 0)
                for field in ("window", "parent_window"):
                    self.assertRegex(dialog[field], r"^0x[0-9a-fA-F]+$")
                self.assertNotEqual(int(dialog["window"],16), int(dialog["parent_window"],16))
            expected_success = closed or operation["open_confirmation"] != "no"
            self.assertEqual(value["succeeded"], expected_success, operation["id"])
            successful += value["succeeded"]
            self.assertEqual(value["selected"], list(range(len(operation["sources"]))) if operation["preselect"] else [])
            has_marker = expected_success and not operation["id"].startswith("unoriented-tetra-")
            self.assertEqual(len(value["points"]), int(has_marker), operation["id"])
            for point in value["points"]:
                self.assertEqual({k:v for k,v in point.items() if k!="point"},
                                 dict(current_layer=True, groups=0, name="", selected=False))
        self.assertEqual((warned, successful), (30,32))
        rows = {row["id"]:row["value"] for row in observed["results"]}
        for operation in data["operations"]:
            name = operation["id"]
            if name.endswith("-escape"):
                yes = rows[name.removesuffix("-escape")+"-yes"]
                self.assertEqual(rows[name]["points"], yes["points"])
        self.assertEqual(rows["open-tetra-pre-yes"]["points"][0]["point"], [.375,.5,1.875])
        self.assertEqual(rows["translated-open-tetra-pre-yes"]["points"][0]["point"], [1000000.375,-1999999.5,3000001.875])
        self.assertEqual(rows["open-quad-pre-yes"]["source_properties"], [dict(volume=-4., centroid=[0.,0.,0.])])
        self.assertEqual(rows["open-quad-pre-yes"]["source_first_moments"], [[-8.,-6.,-2.]])
        self.assertEqual(rows["unoriented-tetra-pre-yes"]["source_properties"][0]["volume"], 0.)
        surface = rows["open-surface-pre-yes"]
        self.assertLess(abs(surface["source_properties"][0]["volume"]), 1e-12)
        self.assertGreater(abs(surface["source_properties"][0]["centroid"][0]), 1e12)
        for actual, expected in zip(surface["points"][0]["point"], [5/3,5/4,1.]):
            self.assertAlmostEqual(actual, expected, delta=1e-9)
        for name in ("unjoined-box-meshes-yes", "unjoined-box-surfaces-yes"):
            box = rows[name]
            self.assertTrue(all(abs(m["volume"]) < 1e-12 for m in box["properties"]))
            for actual, expected in zip(box["points"][0]["point"], [2.,1.5,1.]):
                self.assertAlmostEqual(actual, expected, delta=1e-9)

    def test_provenance_and_failed_attempt_are_retained_separately(self):
        provenance = json.loads((ROOT/"docs/volume-centroid-open-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest)
        failed = (ROOT/"tools/rhino_oracle/observations/volume_centroid_confirmation_attempt.txt").read_text()
        self.assertIn("BadWindow", failed)
        with self.assertRaises(ValueError): json.loads(failed)


if __name__ == "__main__": unittest.main()
