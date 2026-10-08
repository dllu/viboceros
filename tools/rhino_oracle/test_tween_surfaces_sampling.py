"""Sampling construction, isolated native commands and immutable provenance."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleError, OracleProtocolError
from .tween_surfaces_sampling_probe import request, run, validate_request

ROOT=Path(__file__).resolve().parents[2]
class TweenSamplingTests(TestCase):
    def test_closed_recipes_and_private_idle_ownership(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_sampling.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_capture_retains_exact_result_count_and_history(self):
        p=json.loads((ROOT/'docs/tween-sampling-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_sampling.json').read_text())
        self.assertEqual(len(q['results']),20);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        self.assertEqual({r['id']for r in q['results']},{o['id']for o in request()['operations']})
        for row in q['results']:
            v=row['value'];self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active'])
            self.assertEqual(v['undo']['after_script'],v['before']);self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            outputs=v['command']['after_script'][2:]
            self.assertEqual(len(outputs),v['spec']['number']);self.assertEqual(len(v['sampling_sdk']),len(outputs))
            self.assertEqual([o['definition']for o in outputs],v['sampling_sdk'])
            for o in outputs:self.assertEqual(len(o['samples']),81)
