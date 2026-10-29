"""Independent per-face colors, sequential edits and bounded source provenance."""
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from .client import OracleClient,OracleProtocolError
from .generate_subd_merge_properties import request
from .subd_merge_properties_cases import validate
from .structural_sdk import stage_helpers,validate_operations
from .test_merge_faces_command import frozen
from .qualification_sources import SUBD_FACE_PROPERTY_MODULES,with_shared_sources

HERE=Path(__file__).resolve().parent


def record():return json.loads((HERE/'observations/subd_merge_properties.json').read_text())
def properties(rows):
    s=next(r['subd']for r in rows if 'subd'in r)
    return dict(zip(s['face_ids'],s['face_colors']))


class SubDFacePropertiesTests(unittest.TestCase):
    def test_native_archives_complete_requests_and_frozen_provenance(self):
        r=record();q=request();validate_operations(q)
        path=HERE/'fixtures/subd_merge_properties.json'
        self.assertEqual(json.loads(path.read_text()),q)
        self.assertEqual(len(r['results']),20)
        self.assertEqual([o['id']for o in q['operations']],[s['id']for s in r['results']])
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name in r['provenance']['sources']:frozen(r,name)
        archives=0;invocations=0
        for row in r['results']:
            v=row['value']
            for data in [v['source_3dm']]+[s['after_3dm']for s in v['steps']]:
                self.assertTrue(base64.b64decode(data,validate=True).startswith(b'3D Geometry File Format'))
                archives+=1
            for step in v['steps']:
                self.assertEqual(step['command_result'],'Success');invocations+=1
        self.assertEqual((archives,invocations),(41,21))

    def test_merged_colors_clear_while_untouched_and_noop_colors_survive(self):
        for row in record()['results']:
            previous=properties(row['value']['before'])
            for step in row['value']['steps']:
                current=properties(step['after'])
                selected=next(r['selected_faces']for r in step['after']if 'subd'in r)
                for face in selected:self.assertIsNone(current[face],row['id'])
                for face,color in current.items():
                    if face not in selected:self.assertEqual(color,previous[face],row['id'])
                previous=current
        rows={r['id']:r['value']for r in record()['results']}
        for name in ['single-noop','ring-hole-noop']:
            self.assertEqual(properties(rows[name]['before']),properties(rows[name]['steps'][0]['after']))
            self.assertFalse(any(r['name']=='Undo witness'for r in rows[name]['undone'][0]))
        self.assertEqual(properties(rows['strip-same']['steps'][0]['after']),{1:None})

    def test_sequential_undo_and_redo_restore_independent_color_states(self):
        v=next(r['value']for r in record()['results']if r['id']=='strip-sequential')
        self.assertEqual(len(v['steps']),2)
        self.assertEqual(properties(v['undone'][0]),properties(v['steps'][0]['after']))
        self.assertEqual(properties(v['undone'][1]),properties(v['before']))
        for redo,step in zip(v['redone'],v['steps']):self.assertEqual(properties(redo),properties(step['after']))

    def test_closed_cases_and_staged_dependencies(self):
        q=request()
        for op in [None,[],True,{},dict(q['operations'][0],extra=True),dict(q['operations'][0],case=[]),dict(q['operations'][0],id='_Delete')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {
                'subd_merge_properties_probe.py','subd_merge_properties_cases.py','merge_faces_command_cases.py',
                'merge_faces_command_input.py','subd_edit_scene.py','subd_box_probe.py','number_token.py',
                'join_probe.py','merge_edges_probe.py'})

    def test_public_mixed_repeated_and_unbounded_capture_preflight_is_rejected(self):
        q=request()
        for scheme,query in [(None,q),('VibocerosOracleFacePropertiesTest',dict(q,iterations=True)),
                ('VibocerosOracleFacePropertiesTest',dict(q,iterations=2)),
                ('VibocerosOracleFacePropertiesTest',dict(q,operations=q['operations']*4)),
                ('VibocerosOracleFacePropertiesTest',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
            with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                  self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(query))
            launch.assert_not_called()

    def test_current_qualification_binds_every_property_implementation_entry(self):
        root=HERE.parent.parent
        receipt=json.loads((root/'docs/geometry/subd-face-properties-validation.json').read_text())
        self.assertEqual(receipt['native_workflows'],20)
        self.assertEqual(receipt['native_merge_invocations'],21)
        self.assertTrue(set(SUBD_FACE_PROPERTY_MODULES)<=set(receipt['sources']))
        for path in SUBD_FACE_PROPERTY_MODULES:
            self.assertTrue(set(SUBD_FACE_PROPERTY_MODULES)<=set(with_shared_sources([path])),path)
        for name,digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((root/name).read_bytes()).hexdigest(),digest,name)
        for folder,digest in receipt['native_family'].items():
            self.assertEqual(hashlib.sha256((HERE/folder/'subd_merge_properties.json').read_bytes()).hexdigest(),digest)
