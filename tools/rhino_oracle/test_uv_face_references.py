"""Native component references retain source charts and failed input routes."""
import hashlib
import json
from pathlib import Path
from unittest import TestCase,mock
from .uv_face_reference_probe import request,run,validate_request
ROOT=Path(__file__).resolve().parents[2]
def read(path):return json.loads((ROOT/path).read_text())

class UvFaceReferenceTests(TestCase):
    def test_closed_recipes_validate_and_require_owned_idle_document(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/uv_face_reference_command.json'))
        for change in (dict(id='x\n_Exit'),dict(case=[]),dict(case='bad'),dict(extra=True)):
            with self.subTest(change=change),self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=1
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=0;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))

    def test_native_results_preserve_all_face_charts_and_record_rejected_coordinate_routes(self):
        cap=read('tools/rhino_oracle/observations/uv_face_reference_command.json')
        self.assertEqual(len(cap['results']),14)
        good=0
        for row in cap['results']:
            v=row['value'];source=next(o for o in v['before'] if o['kind']=='brep')
            after=next(o for o in v['after'] if o['kind']=='brep')
            self.assertEqual(source['surfaces'],after['surfaces'])
            self.assertEqual(source['face_count'],6)
            output=[o for o in v['after'] if o['source'] is None]
            if v['success']:
                good+=1;self.assertEqual(len(output),1);self.assertEqual(len(output[0]['samples']),33)
                self.assertTrue(all(o['source'] is not None for o in v['undo']))
                self.assertEqual(sum(o['source'] is None for o in v['redo']),1)
            else:
                self.assertIn(row['id'],('uv_face_apply_top','uv_face_apply_front'));self.assertEqual(output,[])
        self.assertEqual(good,12)

    def test_provenance_retains_source_hashes_and_replay_topology_scope(self):
        p=read('docs/uv-face-references-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_component_recipes']),(14,12))
        for path,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
