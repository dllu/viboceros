"""Closed numeric capture keeps point-getter state and every confirmation."""
import hashlib,json,math,os
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_direction_grid_probe import request,validate_request,run
from .subcurve_direction_grid_input import DirectionGridPicker
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(path):return json.loads((ROOT/path).read_text())
class ClosedDirectionTests(TestCase):
    def test_recipes_are_bounded_private_and_require_an_empty_idle_document(self):
        q=request();validate_request(q)
        self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_direction_grid.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True)]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in [dict(protocol_version=True),dict(iterations=True),dict(operations=q['operations']*2),dict(operations=[q['operations'][0]]*2)]:
            with self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        picker=DirectionGridPicker(q)
        self.assertEqual(set(picker.cases),{'@subcurve-direction:'+o['id'] for o in q['operations']})
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_pending_numeric_input_is_a_point_getter_with_no_document_change(self):
        q=read('tools/rhino_oracle/observations/subcurve_direction_grid.json')
        self.assertEqual([r['id'] for r in q['results']],[o['id'] for o in request()['operations']])
        self.assertEqual((len(q['results']),sum(r['value']['success'] for r in q['results'])),(32,26))
        self.assertEqual(sum(bool(r['value']['finish_states']) for r in q['results']),16)
        curves=[o for r in q['results'] for o in r['value']['after'] if o['source'] is None]
        self.assertEqual((len(curves),sum(len(o['samples']) for o in curves)),(28,924))
        for r in q['results']:
            v=r['value'];self.assertFalse(v['command_active']);self.assertEqual(len(v['completions']),1)
            self.assertNotIn('Unknown command',v['history']);self.assertNotIn('Command: _Redo',v['history'])
            self.assertEqual(v['redo'],v['after']);self.assertEqual(len(v['motion']),1)
            self.assertEqual(v['typed_inputs'][0],'D');self.assertEqual(v['typed_inputs'][1],v['length_token'])
            for state in v['finish_states']:
                self.assertTrue(state['in_get_point']);self.assertFalse(state['in_get_object'])
                self.assertEqual(len(state['objects']),len(v['before']))
                for actual,original in zip(state['objects'],v['before']):
                    self.assertEqual(actual['definition'],original['definition']);self.assertFalse(actual['selected'])
    def test_confirmation_can_choose_the_opposite_candidate_on_skew_and_round_curves(self):
        rows={r['value']['case']:r['value'] for r in read('tools/rhino_oracle/observations/subcurve_direction_grid.json')['results']}
        for kind,anchor,direction in [('poly',20,'forward'),('poly',80,'backward'),('circle',20,'forward'),('circle',80,'backward')]:
            base='closed_%s_a%d_%s_l8_'%(kind,anchor,direction)
            same=rows[base+'confirm_numeric'];opposite=rows[base+'opposite_confirm_numeric']
            self.assertTrue(same['success']);self.assertTrue(opposite['success'])
            self.assertEqual(len(same['typed_inputs']),3);self.assertEqual(len(opposite['typed_inputs']),3)
            ends=lambda v:v['after'][1]['samples']
            self.assertGreater(math.dist(ends(same)[0],ends(opposite)[0])+math.dist(ends(same)[-1],ends(opposite)[-1]),1.)
        skew=rows['closed_skew_a20_forward_l8_confirm_numeric']['after'][1]['samples']
        self.assertLess(math.dist(skew[0],[7.191850591615953,.34246907579123587,0.]),1e-9)
        self.assertLess(math.dist(skew[-1],[.8,0.,0.]),1e-9)
        self.assertTrue(rows['inline_closed_poly_a20_forward_l8_opposite_confirm_numeric']['finish_states'][0]['in_get_point'])
    def test_capture_provenance_keeps_current_sources_and_raw_hashes(self):
        p=read('docs/subcurve-direction-confirmation-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual((p['recipes'],p['successful_commands']),(32,26))
        for path,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
