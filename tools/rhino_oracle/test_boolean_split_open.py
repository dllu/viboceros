"""Open planar BooleanSplit target captures and owned recipe checks."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .boolean_split_open_probe import request, validate_request, run
from .client import OracleClient, OracleError, OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]

def read(name):return json.loads((ROOT/name).read_text())

class BooleanSplitOpenTests(TestCase):
    def test_closed_recipes_require_private_idle_empty_context(self):
        q=request();validate_request(q)
        self.assertEqual(q,read('tools/rhino_oracle/fixtures/boolean_split_open.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(op='script'),dict(extra=True)]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_open_outputs_include_cutters_and_preserve_target_metadata(self):
        q=read('tools/rhino_oracle/observations/boolean_split_open.json');success=outputs=0
        self.assertEqual(len(q['results']),16)
        for r in q['results']:
            v=r['value'];before=v['before'];after=v['command']['after_script']
            pieces=[o for o in after if o['source'] is None]
            for i in v['second']:self.assertEqual(next(o for o in after if o['source']==i),dict(before[i],selected=False))
            if not v['command']['success']:
                self.assertEqual(pieces,[]);self.assertEqual(after,[dict(o,selected=False) for o in before]);continue
            success+=1;outputs+=len(pieces)
            for piece in pieces:
                self.assertTrue(piece['valid']);self.assertEqual(piece['selected'],v['pre'])
                for key in ['name','layer','color','attribute_text','geometry_text','groups']:self.assertEqual(piece[key],before[0][key])
            self.assertEqual(v['undo']['after_script'],[dict(o,selected=False) for o in before])
            self.assertEqual(v['redo']['after_script'],after)
        self.assertEqual((success,outputs),(11,27))
    def test_target_normal_changes_the_shared_solid_and_boundaries_are_not_face_only_splits(self):
        cases={r['value']['case']:r['value'] for r in read('tools/rhino_oracle/observations/boolean_split_open.json')['results']}
        for case,x in [('plane_box',.5),('plane_box_reverse',1.5)]:
            pieces=[o for o in cases[case]['command']['after_script'] if o['source'] is None]
            solid=next(o for o in pieces if o['solid']);opened=next(o for o in pieces if not o['solid'])
            self.assertAlmostEqual(solid['volume'],4.);self.assertAlmostEqual(solid['centroid'][0],x)
            self.assertEqual((solid['faces'],opened['faces']),(6,6));self.assertAlmostEqual(opened['area'],24.)
        pieces=[o for o in cases['plane_plane']['command']['after_script'] if o['source'] is None]
        self.assertEqual([o['faces'] for o in pieces],[2,2])
        self.assertTrue(all(not o['solid'] for o in pieces))
    def test_provenance_binds_capture_and_declares_remaining_input_limits(self):
        p=read('docs/boolean-split-open-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands'],p['no_split_failures'],p['outputs']),(16,11,5,27))
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
