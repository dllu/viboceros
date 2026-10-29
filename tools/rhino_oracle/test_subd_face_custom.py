"""Bound authored seeds, immutable Native outputs and current custom-face code."""
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
from .generate_subd_face_custom import request as base
from .generate_subd_face_custom_holdouts import request as holdouts
from .subd_face_custom_cases import CASES,validate
from .subd_face_custom_holdout_cases import CASES as HOLDOUTS,validate as validate_holdout
from .structural_sdk import stage_helpers,validate_operations
from .qualification_sources import SUBD_FACE_CUSTOM_MODULES,with_shared_sources
from .test_merge_faces_command import frozen

HERE=Path(__file__).resolve().parent
FAMILIES=[('subd_face_custom',base,CASES,'subd_face_custom',38),
          ('subd_face_custom_holdouts',holdouts,HOLDOUTS,'subd_face_custom_holdout',14)]


def record(family):return json.loads((HERE/'observations'/(family+'.json')).read_text())


class SubDFaceCustomTests(unittest.TestCase):
    def test_requests_archives_and_all_frozen_producers(self):
        archives=commands=0
        for family,make,_,_,count in FAMILIES:
            q=make();validate_operations(q);r=record(family)
            path=HERE/'fixtures'/(family+'.json')
            self.assertEqual(json.loads(path.read_text()),q)
            self.assertEqual(len(r['results']),count)
            self.assertEqual([s['id']for s in r['results']],[o['id']for o in q['operations']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name in r['provenance']['sources']:frozen(r,name)
            for row in r['results']:
                v=row['value'];outputs=[v['source_3dm']]+[s['after_3dm']for s in v['steps']]
                outputs += [s['archive']for phase in ['undone','redone']for s in v[phase]]
                for output in outputs:
                    self.assertTrue(base64.b64decode(output,validate=True).startswith(b'3D Geometry File Format'))
                    archives+=1
                for step in v['steps']:
                    self.assertEqual(step['command_result'],'Success');commands+=1
        self.assertEqual((archives,commands),(214,54))

    def test_authored_recipes_and_assets_are_distinct_bound_inputs(self):
        for family,_,cases,prefix,_ in FAMILIES:
            r=record(family)
            self.assertEqual(json.loads((HERE/'fixtures'/(prefix+'_recipes.json')).read_text()),cases)
            assets=json.loads((HERE/'fixtures'/(prefix+'_assets.json')).read_text())
            self.assertEqual(set(assets),set(cases))
            for seed in assets.values():
                self.assertEqual(set(seed),{'data','object_name'})
                self.assertEqual(seed['object_name'],'Seed')
                self.assertTrue(base64.b64decode(seed['data'],validate=True).startswith(b'3D Geometry File Format'))
            for name in ['fixtures/'+prefix+'_recipes.json','fixtures/'+prefix+'_assets.json',
                         '../../crates/viboceros-io/examples/subd_face_custom_seeds.rs',
                         'subd_face_custom_cases.py','subd_merge_allocation_cases.py','merge_faces_command_cases.py']:
                self.assertEqual(frozen(r,name),(HERE/name).read_bytes(),name)

    def test_coherent_uvw_holdouts_match_every_shared_control_vertex(self):
        for name,c in HOLDOUTS.items():
            if name not in ['constant-uvw','equal-face-arrays']:
                seen={}
                for face,uvw in zip(c['source']['faces'],c['source']['textures']):
                    for vertex,p in zip(face,uvw):
                        if vertex in seen:self.assertEqual(seen[vertex],p,name)
                        seen[vertex]=p
        self.assertEqual(len(HOLDOUTS['pentagon-and-quad']['source']['faces'][0]),5)
        self.assertEqual(sum('transform'in c for c in HOLDOUTS.values()),4)
        for c in HOLDOUTS.values():self.assertTrue(all(p['material_channel']>0 for p in c['source']['properties']))

    def test_native_texture_sources_and_sequential_history_are_retained(self):
        for family,_,cases,_,_ in FAMILIES:
            for row in record(family)['results']:
                v=row['value'];c=cases[row['id']]
                texture_type=6 if any(t is not None for t in c['source']['textures'])else 2
                for state in [v['before']]+[s['after']for s in v['steps']]+[s['state']for phase in ['undone','redone']for s in v[phase]]:
                    owner=next(o for o in state if 'subd'in o)
                    self.assertEqual(owner['texture_coordinate_type'],texture_type,row['id'])
                self.assertEqual(len(v['undone']),len(v['steps']))
                self.assertEqual(len(v['redone']),len(v['steps']))

    def test_fixed_validators_and_complete_staged_assets(self):
        for make,validator,holdout in [(base,validate,False),(holdouts,validate_holdout,True)]:
            q=make()
            for op in [None,[],True,{},dict(q['operations'][0],extra=True),dict(q['operations'][0],case=[]),dict(q['operations'][0],id='_Delete')]:
                with self.assertRaises(ValueError):validator(op)
            with tempfile.TemporaryDirectory()as directory:
                stage_helpers(q,HERE,Path(directory))
                modules={'subd_face_custom_probe.py','subd_face_custom_cases.py','subd_merge_allocation_cases.py','subd_state.py',
                    'merge_faces_command_cases.py','merge_faces_command_input.py','subd_edit_scene.py','subd_box_probe.py',
                    'number_token.py','join_probe.py','merge_edges_probe.py','fixtures/subd_face_custom_assets.json'}
                if holdout:modules|={'subd_face_custom_holdout_probe.py','subd_face_custom_holdout_cases.py','fixtures/subd_face_custom_holdout_assets.json'}
                actual={str(p.relative_to(directory))for p in Path(directory).rglob('*')if p.is_file()}
                self.assertEqual(actual,modules)
                for name in actual:self.assertEqual((Path(directory)/name).read_bytes(),(HERE/name).read_bytes())

    def test_capture_preflight_rejects_public_mixed_repeated_and_oversized_requests(self):
        for make in [base,holdouts]:
            q=make()
            for scheme,query in [(None,q),('VibocerosOracleCustomTest',dict(q,iterations=True)),
                    ('VibocerosOracleCustomTest',dict(q,iterations=2)),
                    ('VibocerosOracleCustomTest',dict(q,operations=q['operations']*5)),
                    ('VibocerosOracleCustomTest',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
                with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                      mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                      mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                      self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                    OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(query))
                launch.assert_not_called()

    def test_current_receipt_binds_both_native_families_and_shared_source_closure(self):
        root=HERE.parent.parent
        receipt=json.loads((root/'docs/geometry/subd-face-custom-validation.json').read_text())
        self.assertEqual((receipt['native_workflows'],receipt['native_merge_invocations'],receipt['native_archive_models']),(52,54,214))
        self.assertEqual(receipt['coordinate_epsilon'],1e-7)
        self.assertTrue(set(SUBD_FACE_CUSTOM_MODULES)<=set(receipt['sources']))
        for name in SUBD_FACE_CUSTOM_MODULES:
            self.assertTrue(set(SUBD_FACE_CUSTOM_MODULES)<=set(with_shared_sources([name])),name)
        for name,digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((root/name).read_bytes()).hexdigest(),digest,name)
        for family,hashes in receipt['families'].items():
            for folder,digest in hashes.items():
                self.assertEqual(hashlib.sha256((HERE/folder/(family+'.json')).read_bytes()).hexdigest(),digest)
