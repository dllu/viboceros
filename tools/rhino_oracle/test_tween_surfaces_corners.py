"""Calibrated owned corner clicks and complete native geometry evidence."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleError,OracleProtocolError
from .tween_surfaces_corners_probe import request,run,validate_request
from .tween_surfaces_corner_input import CornerPicker
ROOT=Path(__file__).resolve().parents[2]
class TweenCornerTests(TestCase):
    def test_closed_private_idle_click_recipes(self):
        q=request();validate_request(q);self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_corners.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_full_click_calibration_geometry_and_history(self):
        p=json.loads((ROOT/'docs/tween-corners-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_corners.json').read_text())
        self.assertEqual(len(q['results']),9);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for row in q['results']:
            v=row['value'];self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active']);self.assertEqual(v['undo']['after_script'],v['before']);self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            if v['spec']['clicks']:
                self.assertEqual(len(v['click_trace']),1);self.assertEqual(v['click_trace'][0]['pixel'],v['click_pixel']);self.assertEqual(v['calibration']['click_client'],v['click_pixel'])
                matrix=v['calibration']['world_to_screen'];point=v['projection']+[1.];projected=[sum(a*b for a,b in zip(r,point))for r in matrix]
                for axis in (0,1):self.assertAlmostEqual(projected[axis]/projected[3],v['calibration']['aim_client'][axis],places=7)
    def test_driver_requires_complete_owned_click_set(self):
        picker=CornerPicker(request())
        with self.assertRaisesRegex(OracleProtocolError,'incomplete'):picker.record_diagnostics({})
        picker.seen=set(picker.expected);picker.record_diagnostics({})
