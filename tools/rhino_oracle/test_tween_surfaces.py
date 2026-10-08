"""Owned surface tween evidence, including retained refinement duplicates."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleError, OracleProtocolError
from .tween_surfaces_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]

class TweenSurfacesTests(TestCase):
    def test_closed_private_idle_recipes(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_command.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_capture_and_native_refinement_diagnostics(self):
        p=json.loads((ROOT/'docs/tween-surfaces-provenance.json').read_text())
        q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_command.json').read_text())
        self.assertEqual(len(q['results']),22);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for row in q['results']:
            v=row['value'];self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active'])
            self.assertEqual(v['undo']['after_script'],v['before']);self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            self.assertEqual(len(v['sampling_sdk']),v['spec']['number'])
            if v['spec']['method']=='Refit':self.assertEqual(len(v['command']['after_script']),8)
            if v['spec']['method']=='SamplePoints':self.assertEqual(len(v['command']['after_script']),6)
            for obj in v['command']['after_script']:self.assertEqual(len(obj['samples']),81)
