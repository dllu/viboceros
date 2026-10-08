"""Mixed coplanar/open BooleanSplit recipe and native policy evidence."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .boolean_split_mixed_open_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class MixedOpenTests(TestCase):
    def test_owned_recipes_require_private_idle_empty_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/boolean_split_mixed_open.json'))
        for update in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**update)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_success_metadata_retained_sources_and_history(self):
        q=read('tools/rhino_oracle/observations/boolean_split_mixed_open.json');count=0
        self.assertEqual(len(q['results']),20)
        for r in q['results']:
            v=r['value'];self.assertTrue(v['command']['success'],v['case']);after=v['command']['after_script'];before=v['before']
            for i in v['second']:self.assertEqual(next(o for o in after if o['source']==i),dict(before[i],selected=False))
            pieces=[o for o in after if o['source'] is None];count+=len(pieces)
            for piece in pieces:
                self.assertTrue(piece['valid']);self.assertEqual(piece['selected'],v['pre'])
                for key in ['name','layer','color','groups','attribute_text','geometry_text']:self.assertEqual(piece[key],before[0][key])
            self.assertEqual(v['undo']['after_script'],[dict(o,selected=False) for o in before])
            self.assertEqual(v['redo']['after_script'],after)
        self.assertEqual(count,56)
    def test_coplanar_order_containment_and_hole_decisions_are_measured(self):
        cases={r['value']['case']:r['value'] for r in read('tools/rhino_oracle/observations/boolean_split_mixed_open.json')['results']}
        pieces=lambda case:[o for o in cases[case]['command']['after_script'] if o['source'] is None]
        self.assertEqual(len(pieces('coplanar_then_plane')),3);self.assertEqual(len(pieces('plane_then_coplanar')),2)
        self.assertEqual(len(pieces('coplanar_then_box')),4);self.assertEqual(len(pieces('box_then_coplanar')),3)
        self.assertEqual(len(pieces('box_then_straddle_coplanar')),2)
        self.assertEqual(len(pieces('plane_then_inside_coplanar')),3)
        self.assertEqual(len(pieces('inside_coplanar_then_plane')),4)
        self.assertEqual(sorted(round(o['area'],8) for o in pieces('box_then_small_coplanar')),[15.,16.,24.])
        self.assertEqual(sorted(round(o['area'],8) for o in pieces('box_then_large_coplanar')),[7.,16.,24.])
    def test_provenance_binds_native_records_and_retains_limits(self):
        p=read('docs/boolean-split-mixed-open-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands'],p['outputs']),(20,20,56))
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
