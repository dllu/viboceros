"""Bounded public PlanarUnion/Difference/Intersection recipes."""
import json,os
from pathlib import Path
from unittest import TestCase,mock
from .planar_boolean_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
class PlanarBooleanTests(TestCase):
    def test_closed_recipes_and_private_idle_ownership(self):
        q=request();validate_request(q);self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/planar_boolean_command.json').read_text()))
        for change in[dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_capture_metadata_and_provenance(self):
        import hashlib
        p=json.loads((ROOT/'docs/planar-boolean-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/planar_boolean_command.json').read_text())
        self.assertEqual((p['recipes'],p['successful_commands']),(28,28));self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        self.assertEqual({r['id']for r in q['results']},{o['id']for o in request()['operations']})
        for r in q['results']:
            v=r['value'];after=v['command']['after_script'];self.assertTrue(v['command']['success']);self.assertFalse(any(o['selected']for o in after));self.assertEqual(v['redo']['after_script'],after)
            for o in after:
                self.assertIsNone(o['geometry_text'])
                if v['command_name']!='PlanarIntersection':
                    self.assertIsNone(o['source']);self.assertIsNone(o['name']);self.assertIsNone(o['layer']);self.assertEqual(o['groups'],[])
