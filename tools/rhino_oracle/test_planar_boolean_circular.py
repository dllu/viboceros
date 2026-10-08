"""Owned circular planar commands retain curve definitions and diagnostics."""
import json,os,hashlib
from pathlib import Path
from unittest import TestCase,mock
from .planar_boolean_circular_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
class PlanarBooleanCircularTests(TestCase):
    def test_bounded_recipes_and_private_idle_ownership(self):
        q=request();validate_request(q);self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/planar_boolean_circular.json').read_text()))
        for change in[dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_capture_binds_curves_and_explicit_contact_diagnostics(self):
        p=json.loads((ROOT/'docs/planar-boolean-circular-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/planar_boolean_circular.json').read_text())
        self.assertEqual((p['recipes'],p['successful_commands'],p['regular_topology_replays']),(28,28,26));self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual(len(p['diagnostics']),2)
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        self.assertEqual({r['id']for r in q['results']},{o['id']for o in request()['operations']})
        for r in q['results']:
            v=r['value'];after=v['command']['after_script'];self.assertTrue(v['command']['success']);self.assertEqual(v['undo']['after_script'],v['before']);self.assertEqual(v['redo']['after_script'],after)
            for o in after:self.assertEqual(len(o['edge_curves']),o['edges']);self.assertTrue(all(len(points)==33 for points in o['edge_samples']))
