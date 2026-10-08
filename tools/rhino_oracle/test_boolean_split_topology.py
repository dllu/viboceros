"""Trimmed-sheet and compound BooleanSplit capture regressions."""
import hashlib,json,os
from pathlib import Path
from unittest import TestCase,mock
from .boolean_split_topology_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class TopologyTests(TestCase):
    def test_closed_recipes_require_private_idle_empty_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/boolean_split_topology.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_sources_metadata_and_history_preserve_full_outcomes(self):
        q=read('tools/rhino_oracle/observations/boolean_split_topology.json');success=outputs=0;self.assertEqual(len(q['results']),14)
        for r in q['results']:
            v=r['value'];before=v['before'];after=v['command']['after_script'];pieces=[o for o in after if o['source'] is None]
            for i in v['second']:self.assertEqual(next(o for o in after if o['source']==i),dict(before[i],selected=False))
            if not v['command']['success']:
                self.assertEqual(pieces,[]);self.assertEqual(after,[dict(o,selected=False) for o in before]);continue
            success+=1;outputs+=len(pieces)
            for piece in pieces:
                self.assertTrue(piece['valid']);self.assertEqual(piece['selected'],v['pre'])
                for key in ['name','layer','color','groups','attribute_text']:self.assertEqual(piece[key],before[0][key])
                self.assertIn(piece['geometry_text'],[None,before[0]['geometry_text']])
            self.assertEqual(v['undo']['after_script'],[dict(o,selected=False) for o in before]);self.assertEqual(v['redo']['after_script'],after)
        self.assertEqual((success,outputs),(12,33))
    def test_inactive_shells_trim_holes_and_disconnected_lineage_are_measured(self):
        cases={r['value']['case']:r['value'] for r in read('tools/rhino_oracle/observations/boolean_split_topology.json')['results']}
        pieces=lambda c:[o for o in cases[c]['command']['after_script'] if o['source'] is None]
        self.assertAlmostEqual(sum(o['volume'] for o in pieces('compound_target_one_shell')),8.)
        self.assertAlmostEqual(cases['compound_target_one_shell']['before'][0]['volume'],16.)
        self.assertTrue(all(o['geometry_text'] is None for o in pieces('compound_target_both_shells')))
        self.assertEqual(len(pieces('compound_open_target')),4)
        self.assertTrue(all(o['geometry_text'] is None for o in pieces('compound_open_target')))
        self.assertEqual([round(o['area'],8) for o in pieces('sheet_hole_cross_section')],[15.,15.])
        self.assertTrue(all(not o['solid'] for o in pieces('sheet_hole_cross_section')))
        self.assertFalse(cases['open_target_hole_then_coplanar_fill']['command']['success'])
        self.assertEqual(sorted(round(o['area'],8) for o in pieces('open_target_hole_then_coplanar_straddle')),[.25,14.25,15.25])
    def test_provenance_binds_capture_and_declares_remaining_scope(self):
        p=read('docs/boolean-split-topology-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands'],p['no_split_failures'],p['outputs']),(14,12,2,33))
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
