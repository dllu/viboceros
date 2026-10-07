"""SubCrv defaults are immediate and independent of the global Copy switch."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_preferences_probe import request,validate_request,run,STEPS
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class SubcurvePreferencesTests(TestCase):
    def test_closed_request_requires_private_owned_idle_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_preferences.json'))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in [dict(iterations=True),dict(protocol_version=True),dict(operations=q['operations']*2)]:
            with self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_steps_preserve_immediate_defaults_without_filling_hidden_modes(self):
        rows=read('tools/rhino_oracle/observations/subcurve_preferences.json')['results'][0]['value']['records'];self.assertEqual(len(rows),37);self.assertEqual([r['step'] for r in rows],list(STEPS))
        self.assertEqual(rows[0]['choices'],dict(Copy='No',FromMidpoint='No',Mode='Shorten'))
        self.assertEqual(rows[2]['choices']['Copy'],'Yes');self.assertEqual(rows[6]['choices']['Copy'],'Yes');self.assertEqual(rows[30]['choices']['Copy'],'Yes')
        self.assertEqual(rows[25]['choices']['Mode'],'MarkEnds');self.assertEqual(rows[26]['after'][1]['kind'],'point');self.assertEqual(rows[34]['choices']['FromMidpoint'],'Yes');self.assertEqual(rows[36]['choices']['FromMidpoint'],'No')
        self.assertIsNone(rows[2]['choices']['Mode'])
        for row in rows:
            self.assertFalse(row['command_active'])
            if row['step'].get('query') or row['step'].get('finish')=='Cancel':
                self.assertFalse(row['success']);self.assertEqual(row['after'],[dict(o,selected=False) for o in row['before']])
    def test_provenance_keeps_canonical_producer_and_native_records(self):
        p=read('docs/subcurve-option-memory-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual(p['workflow_steps'],37)
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
