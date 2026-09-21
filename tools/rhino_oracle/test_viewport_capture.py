"""Owned-click calibration, strict snap grammar and persistent setting restoration."""
from types import SimpleNamespace
import unittest
from . import viewport_capture, split_edge_probe


class ViewportCaptureTests(unittest.TestCase):
    def test_persistent_modes_are_set_and_original_settings_restored_after_failure(self):
        class Settings:
            def GetCurrentState(self): return self.__dict__.copy()
            def UpdateFromState(self, state): self.__dict__.clear(); self.__dict__.update(state)
        aid, track = Settings(), Settings()
        aid.OsnapModes, aid.Untouched, track.UseSmartTrack = 123, "keep", True
        original_aid, original_track = aid.GetCurrentState(), track.GetCurrentState()
        modes = SimpleNamespace(**{"None":0,"Point":134217728,"End":131072,"Midpoint":2048,"Center":32,"Quadrant":512})
        host = dict(Rhino=SimpleNamespace(ApplicationSettings=SimpleNamespace(ModelAidSettings=aid,SmartTrackSettings=track,OsnapModes=modes)))
        operation = dict(inputs=[dict(pick={})],persistent_snaps=["Point","End","Mid","Cen","Quad"])
        with self.assertRaisesRegex(ValueError,"command failed"):
            with split_edge_probe.snapping_environment(operation,host):
                self.assertEqual(aid.OsnapModes,134217728|131072|2048|32|512)
                self.assertFalse(track.UseSmartTrack)
                raise ValueError("command failed")
        self.assertEqual(aid.GetCurrentState(),original_aid)
        self.assertEqual(track.GetCurrentState(),original_track)

    def setup_projection(self):
        matrix = [[2.,0.,0.,10.],[0.,-3.,0.,20.],[0.,0.,1.,0.],[0.,0.,0.,1.]]
        class Transform:
            def __getitem__(self, key): return matrix[key[0]][key[1]]
        host = dict(Rhino=SimpleNamespace(DocObjects=SimpleNamespace(
            CoordinateSystem=SimpleNamespace(World=1,Screen=2))), _xyz=lambda p: list(p))
        view = SimpleNamespace(GetTransform=lambda a,b: Transform(),
            WorldToClient=lambda p: SimpleNamespace(X=p[0]*2+10,Y=-p[1]*3+20),
            Size=SimpleNamespace(Width=800,Height=600),CameraLocation=[0,0,10],
            CameraDirection=[0,0,-1],IsPerspectiveProjection=False)
        return matrix, host, view

    def test_public_matrix_agrees_with_independent_client_projection(self):
        matrix, host, view = self.setup_projection()
        result = viewport_capture.capture(view,[3,4,0],[21,9],host)
        self.assertEqual(result["world_to_screen"],matrix)
        self.assertEqual(result["aim_client"],[16.,8.])
        self.assertEqual(result["click_client"],[21,9])
        self.assertEqual(result["camera_direction"],[0,0,-1])
        self.assertEqual(result["size"],[800,600])
        self.assertFalse(result["perspective"])

    def test_invalid_or_inconsistent_transforms_fail_closed(self):
        for failure in ("nonfinite","plane","mismatch","client","camera","direction"):
            matrix, host, view = self.setup_projection()
            if failure == "nonfinite": matrix[0][0] = float("nan")
            elif failure == "plane": matrix[3][3] = 0.
            elif failure == "client": view.WorldToClient = lambda p: SimpleNamespace(X=float("nan"),Y=8.)
            elif failure == "camera": view.CameraLocation = [float("inf"),0,10]
            elif failure == "direction": view.CameraDirection = [0,0,0]
            else: matrix[0][3] += 1.
            with self.subTest(failure=failure), self.assertRaises(ValueError):
                viewport_capture.capture(view,[3,4,0],[16,8],host)

    def test_persistent_modes_and_separate_aim_are_bounded_before_host_access(self):
        operation = dict(op="split_edge_command", id="calibrated",edge=0,pick="mouse",selected=[0],
            sources=[dict(brep=dict(artifact_path="/owned/box.3dm"))],record_viewport=True,
            persistent_snaps=["Point","End","Mid","Cen","Quad"],
            inputs=[dict(pick=dict(point=[4,-4,0],aim=[2.4,-2.8,0],osnap="Persistent"))])
        self.assertEqual(split_edge_probe.mouse_command(operation,None,{}),"_SplitEdge _Pause _Pause _Enter")
        for value in (None, "Cen", ["Cen","Cen"], ["_Delete"], ["NoSnap"], ["Persistent"], [{}]):
            with self.subTest(value=value), self.assertRaises(ValueError):
                split_edge_probe.run(dict(operation,persistent_snaps=value),None,{})
        for aim in (None, [0,0], [True,0,0], [float("inf"),0,0]):
            updated = dict(operation,inputs=[dict(pick=dict(operation["inputs"][0]["pick"],aim=aim))])
            with self.subTest(aim=aim), self.assertRaises(ValueError): split_edge_probe.run(updated,None,{})
        for value in (1, "True", None):
            with self.assertRaises(ValueError): split_edge_probe.run(dict(operation,record_viewport=value),None,{})
