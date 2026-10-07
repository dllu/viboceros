"""MarkEnds creates default unselected points and preserves original curves."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_mark_ends_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class MarkEndsTests(TestCase):
    def test_closed_recipes_and_owned_idle_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_mark_ends.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_marker_attributes_source_purity_and_history(self):
        q=read('tools/rhino_oracle/observations/subcurve_mark_ends.json');self.assertEqual(len(q['results']),15);self.assertEqual(sum(r['value']['success'] for r in q['results']),13)
        count=0
        for r in q['results']:
            v=r['value'];self.assertFalse(v['command_active']);source=v['before'][0]
            self.assertEqual(v['after'][0],dict(source,selected=False))
            self.assertEqual(len(v['undo']),1);self.assertEqual(v['undo'][0],dict(source,selected=False))
            for marker in v['after'][1:]:
                count+=1;self.assertIn('point',marker);self.assertFalse(marker['selected']);self.assertEqual(marker['groups'],[]);self.assertIsNone(marker['name']);self.assertIsNone(marker['user_text']);self.assertTrue(marker['layer'].endswith('_output'))
            self.assertEqual(v['redo'],v['after'])
        self.assertEqual(count,26)
    def test_provenance_keeps_native_cancellations_and_current_capture_hashes(self):
        p=read('docs/subcurve-mark-ends-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual((p['recipes'],p['successful_commands'],p['markers']),(15,13,26));self.assertEqual(p['retained_cancellations'],['zero','no_confirmation'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
