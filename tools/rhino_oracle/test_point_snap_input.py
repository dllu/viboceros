"""Motion precedes delayed clicks, without blocking or using foreign windows."""
import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from .client import OracleProtocolError
from .point_snap_input import PointSnapPicker
from .point_snap_replay import prepare
from .references.point_snaps import request
from .test_point_snap_replay import inputs


class PointSnapInputTests(unittest.TestCase):
    def picker(self,delay=250):
        data=request(); data["operations"] = data["operations"][:1]
        data["operations"][0]["input_settle_ms"] = delay
        picker=PointSnapPicker(data)
        name="@point:"+data["operations"][0]["id"]
        picker.ready[name]=0
        return picker,name

    def test_delayed_motion_is_nonblocking_and_acknowledges_only_one_later_click(self):
        picker,name=self.picker()
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory); (job/"worker-progress.log").write_text("PICK %s 123 456\n" % name)
            with patch("tools.rhino_oracle.group_picking._rhino_window_for_pids",return_value="owned"), \
                 patch("tools.rhino_oracle.point_snap_input.subprocess.run") as run, \
                 patch("tools.rhino_oracle.point_snap_input.time.monotonic",return_value=2.) as now:
                picker(job,set()); run.assert_not_called()
                picker(job,{17})
                run.assert_called_once_with(["xdotool","windowactivate","--sync","owned","mousemove","124","456","mousemove","123","456"],check=True,timeout=10)
                self.assertFalse((job/"click-ack.json").exists())
                now.return_value=2.24; picker(job,{17}); self.assertEqual(run.call_count,1)
                now.return_value=2.26; picker(job,{17}); picker(job,{17})
                self.assertEqual(run.call_count,2)
                run.assert_called_with(["xdotool","windowactivate","--sync","owned","click","1"],check=True,timeout=10)
                self.assertEqual(json.loads((job/"click-ack.json").read_text()),name)
                self.assertGreaterEqual(picker.events[name]["motion_to_click_ms"],250)
                response=dict(results=[dict(id=name[7:],value=dict(point=[1,2,3]))])
                picker.record_diagnostics(response)
                self.assertEqual(response["results"][0]["value"]["point"],[1,2,3])
                self.assertEqual(response["results"][0]["value"]["input_motion"],picker.events[name])

    def test_window_target_changes_and_failed_motion_do_not_click_or_acknowledge(self):
        for failure in ("window","target","motion","click"):
            picker,name=self.picker()
            with patch("tools.rhino_oracle.point_snap_input.subprocess.run") as run, \
                 patch("tools.rhino_oracle.point_snap_input.time.monotonic",return_value=2.) as now:
                if failure=="motion":
                    run.side_effect=subprocess.TimeoutExpired("xdotool",10)
                    with self.assertRaises(subprocess.TimeoutExpired): picker.send_input(name,"123","456","owned")
                    self.assertFalse(picker.moved)
                else:
                    self.assertFalse(picker.send_input(name,"123","456","owned"))
                    now.return_value=3.
                    if failure=="click":
                        run.side_effect=subprocess.TimeoutExpired("xdotool",10)
                        with self.assertRaises(subprocess.TimeoutExpired): picker.send_input(name,"123","456","owned")
                    else:
                        with self.assertRaises(OracleProtocolError):
                            picker.send_input(name,"124" if failure=="target" else "123","456","foreign" if failure=="window" else "owned")
                        self.assertEqual(run.call_count,1)
                self.assertFalse(picker.events)
                self.assertNotIn(name,picker.seen)
                with self.assertRaises(OracleProtocolError): picker.record_diagnostics(dict(results=[dict(id=name[7:],value={})]))

    def test_unknown_marker_is_not_an_authorized_click(self):
        picker,_=self.picker()
        with patch("tools.rhino_oracle.point_snap_input.subprocess.run") as run:
            with self.assertRaises(OracleProtocolError): picker.send_input("@point:foreign","1","2","owned")
            run.assert_not_called()

    def test_abort_after_motion_never_delivers_the_pending_click(self):
        picker,name=self.picker()
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory); progress=job/"worker-progress.log"
            progress.write_text("PICK %s 123 456\n" % name)
            with patch("tools.rhino_oracle.group_picking._rhino_window_for_pids",return_value="owned"), \
                 patch("tools.rhino_oracle.point_snap_input.subprocess.run") as run, \
                 patch("tools.rhino_oracle.point_snap_input.time.monotonic",return_value=2.) as now:
                picker(job,{17})
                progress.write_text("PICK %s 123 456\nPICK_ABORT %s\n" % (name,name[7:]))
                now.return_value=3.; picker(job,{17}); picker(job,{17})
                self.assertEqual(run.call_count,2)
                run.assert_called_with(["xdotool","windowactivate","--sync","owned","key","--clearmodifiers","Escape"],check=True,timeout=10)
                self.assertFalse((job/"click-ack.json").exists())
                self.assertFalse(picker.events)

    def test_host_diagnostics_reject_wrong_ids_duplicate_ids_and_existing_fields(self):
        picker,name=self.picker()
        for rows in ([],[dict(id="foreign",value={})],[dict(id=name[7:],value={})]*2,
                     [dict(id=name[7:],value=dict(input_motion={}))]):
            response=dict(results=copy.deepcopy(rows)); original=copy.deepcopy(response)
            with self.assertRaises(OracleProtocolError): picker.record_diagnostics(response)
            self.assertEqual(response,original)

    def test_replay_requires_matching_host_evidence_but_never_uses_timing_as_native_input(self):
        data,observed=inputs()
        original=prepare(data,observed)[0]
        data["operations"][0]["input_settle_ms"]=250
        with self.assertRaises(OracleProtocolError): prepare(data,observed)
        motion=dict(requested_settle_ms=250,motion_to_click_ms=260.,detour_pixels=1)
        observed["results"][0]["value"]["input_motion"]=motion
        self.assertEqual(prepare(data,observed)[0],original)
        for changes in (dict(requested_settle_ms=200),dict(motion_to_click_ms=249.99),dict(motion_to_click_ms=float("nan")),dict(detour_pixels=True)):
            bad=copy.deepcopy(observed); bad["results"][0]["value"]["input_motion"].update(changes)
            with self.assertRaises(OracleProtocolError): prepare(data,bad)


if __name__ == "__main__": unittest.main()
