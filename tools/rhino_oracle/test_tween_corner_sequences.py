"""Bounded real multi-click surface direction sequences and native evidence."""
import base64,hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleError,OracleProtocolError
from .tween_surfaces_corner_sequences_probe import request,run,validate_request,SPECS
from .tween_surfaces_corner_sequence_input import SequencePicker
ROOT=Path(__file__).resolve().parents[2]
class TweenSequenceTests(TestCase):
    def test_closed_private_idle_sequences(self):
        q=request();validate_request(q);self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_corner_sequences.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_full_multi_click_calibration_geometry_history_and_provenance(self):
        p=json.loads((ROOT/'docs/tween-sequences-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_corner_sequences.json').read_text())
        self.assertEqual(len(q['results']),14);self.assertFalse(p['full_native_parity'])
        clicks=0
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for row in q['results']:
            v=row['value'];self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active']);self.assertEqual(v['undo']['after_script'],v['before']);self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            self.assertEqual(len(v['click_trace']),len(v['spec']['clicks']));self.assertEqual(len(v['calibrations']),len(v['spec']['clicks']))
            clicks+=len(v['click_trace'])
            for event,calibration in zip(v['click_trace'],v['calibrations']):
                self.assertEqual(event['pixel'],calibration['click_client'])
                point=calibration['aim']+[1.];mapped=[sum(a*b for a,b in zip(r,point))for r in calibration['world_to_screen']]
                for axis in (0,1):self.assertAlmostEqual(mapped[axis]/mapped[3],calibration['aim_client'][axis],places=7)
        self.assertEqual(clicks,p['calibrated_mouse_clicks'])
    def test_driver_expects_every_bounded_click_and_initial_diagnostics_are_intact(self):
        picker=SequencePicker(request())
        self.assertEqual(len(picker.expected),sum(len(s['clicks'])for s in SPECS.values()))
        with self.assertRaisesRegex(OracleProtocolError,'incomplete'):picker.record_diagnostics({})
        picker.seen=set(picker.expected);picker.record_diagnostics({})
        d=json.loads((ROOT/'tools/rhino_oracle/observations/tween_corner_sequence_driver_initial.json').read_text())
        for value in d['files'].values():self.assertEqual(hashlib.sha256(base64.b64decode(value['base64'])).hexdigest(),value['sha256'])
