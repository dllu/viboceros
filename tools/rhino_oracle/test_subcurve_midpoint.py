"""FromMidpoint captures use half-lengths and retain cancellations unaltered."""
import hashlib,json,math,os
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_midpoint_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class MidpointTests(TestCase):
    def test_bounded_recipes_and_owned_idle_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_midpoint.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True)]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in [dict(protocol_version=True),dict(iterations=True),dict(operations=q['operations']*2)]:
            with self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_half_length_endpoints_metadata_and_history(self):
        q=read('tools/rhino_oracle/observations/subcurve_midpoint.json');self.assertEqual(len(q['results']),22);self.assertEqual(sum(r['value']['success'] for r in q['results']),19)
        rows={r['id'].removeprefix('midpoint_'):r['value'] for r in q['results']}
        self.assertEqual([name for name,v in rows.items() if not v['success']],['closed_full','zero','closed_half'])
        result=rows['forward_copy']['after'][1];self.assertAlmostEqual(math.dist(result['samples'][0],result['samples'][-1]),4.)
        self.assertLess(math.dist(rows['clamp_start']['after'][1]['samples'][0],[0.,0.,0.]),1e-9)
        self.assertLess(math.dist(rows['clamp_end']['after'][1]['samples'][-1],[4.,6.,0.]),1e-9)
        for name,v in rows.items():
            self.assertFalse(v['command_active']);self.assertEqual(len(v['undo']),1);self.assertFalse(v['undo'][0]['selected']);self.assertEqual(v['undo'][0]['definition'],v['before'][0]['definition'])
            if v['copy'] or name.startswith('mark_') or not v['success']:self.assertEqual(v['after'][0],dict(v['before'][0],selected=False))
            for o in v['after'][1:]:
                if 'point' in o:self.assertFalse(o['selected']);self.assertEqual(o['groups'],[]);self.assertTrue(o['layer'].endswith('_output'))
            self.assertEqual(v['redo'],v['after'])
    def test_provenance_keeps_current_source_and_raw_geometry_hashes(self):
        p=read('docs/subcurve-midpoint-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual((p['recipes'],p['successful_commands']),(22,19))
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
