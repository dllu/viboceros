"""Standalone SubCrv geometry, metadata and history without output normalization."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .standalone_subcurve_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class StandaloneSubcurveTests(TestCase):
    def test_closed_recipes_require_owned_idle_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/standalone_subcurve.json'))
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
    def test_native_copy_replace_geometry_selection_metadata_and_history(self):
        q=read('tools/rhino_oracle/observations/standalone_subcurve.json');self.assertEqual(len(q['results']),17)
        self.assertEqual(sum(r['value']['success'] for r in q['results']),15)
        for r in q['results']:
            v=r['value'];self.assertFalse(v['command_active'])
            source=v['before'][0]
            for o in v['after']:
                self.assertEqual((o['name'],o['layer'],o['user_text'],o['groups']),(source['name'],source['layer'],source['user_text'],source['groups']))
                self.assertEqual(len(o['samples']),33)
            if not v['success']:
                self.assertEqual(len(v['after']),1);self.assertFalse(v['after'][0]['selected']);continue
            if v['copy']:
                self.assertEqual(v['after'][0],dict(source,selected=False));self.assertTrue(v['after'][1]['selected'])
            else:self.assertFalse(v['after'][0]['selected'])
            self.assertEqual(len(v['undo']),1);self.assertFalse(v['undo'][0]['selected'])
            self.assertEqual(v['undo'][0]['definition'],source['definition'])
            self.assertEqual([(o['definition'],o['selected']) for o in v['redo']],[(o['definition'],o['selected']) for o in v['after']])
    def test_provenance_preserves_native_failures_and_current_sources(self):
        p=read('docs/standalone-subcurve-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands']),(17,15));self.assertEqual(p['retained_cancellations'],['zero','no_confirmation'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
