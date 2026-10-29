"""Frozen getter inputs, acknowledged clicks and Native allocation variation."""
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from .client import OracleClient,OracleProtocolError
from .capture_preflight_components import prepare
from .generate_subd_insert_custom import request as base
from .generate_subd_insert_order import request as holdouts
from .subd_insert_custom_cases import CASES,validate
from .subd_insert_order_cases import CASES as HOLDOUTS,validate as validate_holdout
from .structural_sdk import stage_helpers,validate_operations
from .test_merge_faces_command import frozen

HERE=Path(__file__).resolve().parent
FAMILIES=[('subd_insert_custom',base,CASES,22),('subd_insert_order',holdouts,HOLDOUTS,16),
          ('subd_insert_order_repeat',holdouts,HOLDOUTS,16)]
def record(family):return json.loads((HERE/'observations'/(family+'.json')).read_text())
def definition(state):return next(o['subd']for o in state if 'subd'in o)


class SubDInsertCustomTests(unittest.TestCase):
    def test_all_requests_producers_archives_and_getter_clicks_are_bound(self):
        archives=0
        for family,make,cases,count in FAMILIES:
            q=make();validate_operations(q);r=record(family)
            fixture=HERE/'fixtures'/(family+'.json')
            self.assertEqual(json.loads(fixture.read_text()),q)
            self.assertEqual(len(r['results']),count)
            self.assertEqual([o['id']for o in q['operations']],[o['id']for o in r['results']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name in r['provenance']['sources']:frozen(r,name)
            case_file='subd_insert_custom_cases.py'if make is base else'subd_insert_order_cases.py'
            namespace={};exec(compile(frozen(r,case_file),case_file,'exec'),namespace)
            self.assertEqual(namespace['CASES'],cases)
            recipe=family.replace('_repeat','')+'_recipes.json'
            self.assertEqual(json.loads((HERE/'fixtures'/recipe).read_text()),cases)
            for row in r['results']:
                v=row['value'];case=cases[row['id']]
                self.assertEqual(len(v['trace']['clicks']),sum(isinstance(a,dict)for a in case['actions']))
                self.assertTrue(all(c['click_acknowledged']and c['motion_acknowledged']for c in v['trace']['clicks']))
                self.assertEqual(v['trace']['errors'],[])
                self.assertTrue(v['trace']['completed'])
                outputs=[v[k]for k in ['initial_3dm','source_3dm','after_3dm']]
                if v['merge']:outputs.append(v['merge']['archive'])
                outputs += [s['archive']for phase in ['undone','redone']for s in v[phase]]
                for output in outputs:
                    self.assertTrue(base64.b64decode(output,validate=True).startswith(b'3D Geometry File Format'))
                    archives+=1
                self.assertEqual(len(v['undone']),len(v['redone']))
        self.assertEqual(archives,332)

    def test_repeat_proves_new_ids_can_vary_with_identical_source_and_inputs(self):
        a=record('subd_insert_order')['results'];b=record('subd_insert_order_repeat')['results']
        changed=[]
        for first,second in zip(a,b):
            self.assertEqual(first['id'],second['id'])
            left=definition(first['value']['before']);right=definition(second['value']['before'])
            for key in ['vertex_ids','points','edges','edge_ids','faces','face_ids']:
                self.assertEqual(left[key],right[key],(first['id'],key))
            left=definition(first['value']['after']);right=definition(second['value']['after'])
            def vertices(s):return sorted(zip(s['vertex_ids'],s['points']))
            if vertices(left)!=vertices(right):changed.append(first['id'])
            def shape(s):
                points=[tuple(p)for p in s['points']]
                def cycle(face):
                    p=[points[i]for i in face]
                    return min(tuple(p[i:]+p[:i])for i in range(len(p)))
                return sorted(points),sorted(cycle(f)for f in s['faces'])
            self.assertEqual(shape(left),shape(right),first['id'])
        self.assertEqual(changed,['grid-reverse-three','merged-grid-reverse'])

    def test_disconnected_pick_is_rejected_and_undo_removes_last_accepted_point(self):
        rows={r['id']:r['value']for r in record('subd_insert_order')['results']}
        for name,added,result in [('disjoint-left-prefix',1,'Success'),('disjoint-reverse',1,'Success'),
                                  ('disjoint-return',2,'Success'),('disjoint-reject-undo',0,'Nothing')]:
            v=rows[name]
            self.assertEqual(v['events'],[result])
            self.assertEqual(len(definition(v['after'])['points'])-len(definition(v['before'])['points']),added)

    def test_private_dedicated_capture_policy_and_owned_case_validation(self):
        for make,validator in [(base,validate),(holdouts,validate_holdout)]:
            q=make()
            for op in [None,[],True,{},dict(q['operations'][0],case=[]),dict(q['operations'][0],extra=True),dict(q['operations'][0],id='_Delete')]:
                with self.assertRaises(ValueError):validator(op)
            for scheme,query in [(None,q),('VibocerosOracleInsertTest',dict(q,iterations=True)),('VibocerosOracleInsertTest',dict(q,iterations=2)),
                    ('VibocerosOracleInsertTest',dict(q,operations=q['operations']*5)),
                    ('VibocerosOracleInsertTest',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
                with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                      mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                      mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                      self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                    OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(query))
                launch.assert_not_called()
            with mock.patch('tools.rhino_oracle.catenary_point_picker.CatenaryPointPicker')as picker:
                prepare(SimpleNamespace(settings_scheme='VibocerosOracleInsertTest'),q,None)
                self.assertEqual(picker.call_args.args[0],dict(operations=[dict(id=o['id'],actions=(CASES if make is base else HOLDOUTS)[o['case']]['actions'])for o in q['operations']]))

    def test_worker_staging_contains_every_probe_and_both_owned_seed_sets(self):
        for make in [base,holdouts]:
            with tempfile.TemporaryDirectory()as directory:
                stage_helpers(make(),HERE,Path(directory))
                staged={str(p.relative_to(directory))for p in Path(directory).rglob('*')if p.is_file()}
                required={'subd_insert_custom_probe.py','subd_insert_custom_cases.py','subd_state.py','subd_edit_scene.py',
                          'conic_getter_input.py','managed_mouse_callback.py','snap_environment.py',
                          'merge_faces_command_input.py','fixtures/subd_face_custom_assets.json',
                          'fixtures/subd_face_custom_holdout_assets.json'}
                if make is holdouts:required|={'subd_insert_order_probe.py','subd_insert_order_cases.py'}
                self.assertTrue(required<=staged)
                for name in staged:self.assertEqual((Path(directory)/name).read_bytes(),(HERE/name).read_bytes())

    def test_current_receipt_binds_implementation_and_both_repeat_outputs(self):
        from .qualification_sources import SUBD_INSERT_MODULES,with_shared_sources
        receipt=json.loads((HERE.parent.parent/'docs/geometry/subd-insert-validation.json').read_text())
        self.assertEqual((receipt['native_workflows'],receipt['native_archive_models']),(54,332))
        self.assertEqual(receipt['coordinate_epsilon'],1e-7)
        for name in SUBD_INSERT_MODULES:self.assertTrue(set(SUBD_INSERT_MODULES)<=set(with_shared_sources([name])))
        for name,digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((HERE.parent.parent/name).read_bytes()).hexdigest(),digest,name)
        for family,_,_,_ in FAMILIES:
            for folder,digest in receipt['families'][family].items():
                self.assertEqual(hashlib.sha256((HERE/folder/(family+'.json')).read_bytes()).hexdigest(),digest)
