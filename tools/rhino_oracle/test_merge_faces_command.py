"""Owned command captures, physical input acknowledgments and replay provenance."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from .client import OracleClient, OracleProtocolError
from .generate_merge_faces_command import request
from .generate_merge_faces_command_more import request as more
from .generate_merge_faces_command_order import request as order
from .merge_faces_command_cases import CASES, EXTRA_CASES, ORDER_CASES, validate
from .qualification_sources import MERGE_COMMAND_MODULES, with_shared_sources
from .structural_sdk import stage_helpers, validate_operations

HERE=Path(__file__).resolve().parent
FAMILIES=[('merge_faces_command',request,31),('merge_faces_command_more_v2',more,14),
          ('merge_faces_command_order',order,32)]


def observation(family):
    return json.loads((HERE/'observations'/(family+'.json')).read_text())


def frozen(record,name):
    digest=record['provenance']['sources'][name]
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=digest:
        data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
    if hashlib.sha256(data).hexdigest()!=digest:raise ValueError('producer digest: '+name)
    return data


class MergeFacesCommandTests(unittest.TestCase):
    def test_fixed_requests_archives_and_frozen_producers(self):
        total=0
        for family,make,count in FAMILIES:
            query=make();validate_operations(query)
            path=HERE/'fixtures'/(family+'.json')
            self.assertEqual(json.loads(path.read_text()),query)
            record=observation(family)
            self.assertEqual(len(record['results']),count)
            total+=count
            self.assertEqual([r['id']for r in record['results']],[o['id']for o in query['operations']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),record['provenance']['request_sha256'])
            for name in record['provenance']['sources']:frozen(record,name)
            for row in record['results']:
                for name in ['source_3dm','after_3dm']:
                    self.assertIsInstance(row['value'][name],str)
                    self.assertGreater(len(row['value'][name]),100)
        self.assertEqual(total,77)

    def test_replay_recipes_are_bound_to_the_original_scene_builders(self):
        recipes=json.loads((HERE/'fixtures/merge_faces_command_recipes.json').read_text())
        self.assertEqual(recipes,dict(CASES,**EXTRA_CASES,**ORDER_CASES))
        current=(HERE/'merge_faces_command_cases.py').read_text()
        for family,_,_ in FAMILIES:
            original=frozen(observation(family),'merge_faces_command_cases.py').decode()
            if family=='merge_faces_command':
                original=original.split('def validate')[0].split('EXTRA_CASES =')[0].strip()
                expected=current.split('EXTRA_CASES =')[0].strip()
            elif family.endswith('_v2'):
                original=original.split('def validate')[0].strip()
                expected=current.split('# Repeat identical public')[0].strip()
            else:
                original=original.split('def validate')[0].strip()
                expected=current.split('def validate')[0].strip()
            self.assertEqual(original,expected,family)

    def test_physical_postselection_is_acknowledged_by_the_active_getter(self):
        rows=[r for r in observation('merge_faces_command_more_v2')['results']if '-postselected-'in r['id']]
        self.assertEqual(len(rows),4)
        for row in rows:
            interaction=row['value']['interaction']
            self.assertEqual(interaction['component_admitted'],[True,True])
            self.assertEqual(interaction['getter_counts'],[1,2])
            self.assertEqual(interaction['owner_promoted'],[False,False])
            self.assertTrue(interaction['entered']);self.assertTrue(interaction['command_success'])
            self.assertEqual(interaction['command_count'],1)

    def test_previous_capture_is_retained_as_a_diagnostic_not_an_archive_seed(self):
        record=observation('merge_faces_command_more')
        self.assertEqual(len(record['results']),14)
        bad=[r['id']for r in record['results']if not isinstance(r['value']['source_3dm'],str)]
        self.assertEqual(set(bad),{kind+'-postselected-'+result for kind in ['mesh','subd']for result in ['Selected','NotSelected']})
        for name in record['provenance']['sources']:frozen(record,name)

    def test_same_native_mesh_identities_can_have_opposite_insertion_orders(self):
        rows=observation('merge_faces_command_order')['results']
        samples=[r['value']for r in rows if r['id'].startswith('mesh-fixed-ids-NotSelected')]
        self.assertEqual(len(samples),8)
        ids=lambda v:tuple(r['object_id']for r in v['before']if r['source']is not None)
        before=lambda v:tuple((r['name'],r['geometry'])for r in v['before']if r['source']is not None)
        self.assertTrue(all(ids(v)==ids(samples[0])and before(v)==before(samples[0])for v in samples))
        orders={tuple(r['source']for r in v['after']if r['source']is not None)for v in samples}
        self.assertEqual(orders,{(0,1),(1,0)})
        for v in samples:
            self.assertEqual([r['source']for r in v['after']],[r['source']for r in v['redone']])
            self.assertEqual([r['source']for r in v['before']],[r['source']for r in v['undone']])

    def test_results_selection_noop_history_and_unrelated_whole_selection(self):
        rows={r['id']:r['value']for family,_,_ in FAMILIES for r in observation(family)['results']}
        for name,v in rows.items():
            if 'NotSelected'in name:
                self.assertTrue(all(not r['components']for r in v['after']))
            self.assertTrue(all(not r['components']for r in v['undone']))
            self.assertTrue(all(c['kind']in('MeshFace','SubdFace')for r in v['redone']for c in r['components']))
        for result in ['Selected','NotSelected']:
            v=rows['wrong-owner-peer-'+result]
            for phase in ['after','undone','redone']:
                self.assertTrue(next(r for r in v[phase]if r['name']=='Undo witness')['selected'])
            for kind in ['mesh','subd']:
                v=rows[kind+'-single-'+result]
                self.assertFalse(any(r['name']=='Undo witness'for r in v['undone']))
                self.assertTrue(any(r['name']=='Undo witness'for r in v['redone']))

    def test_closed_cases_helpers_and_private_bounded_preflight(self):
        for op in [None,[],True,{},dict(op='merge_faces_command',id='bad',case='_Delete'),
                   dict(request()['operations'][0],extra=True),dict(request()['operations'][0],case=[])]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(),HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {
                'merge_faces_command_probe.py','merge_faces_command_cases.py','merge_faces_command_input.py',
                'managed_component_input.py','subd_edit_scene.py','subd_box_probe.py','number_token.py',
                'join_probe.py','merge_edges_probe.py'})
        q=request()
        for scheme,query in [(None,q),('VibocerosOracleMergeTest',dict(q,iterations=True)),
                ('VibocerosOracleMergeTest',dict(q,iterations=2)),
                ('VibocerosOracleMergeTest',dict(q,operations=q['operations']*3)),
                ('VibocerosOracleMergeTest',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
            with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                  self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(query))
            launch.assert_not_called()

    def test_shared_implementation_inventory_and_current_receipt(self):
        root=HERE.parent.parent
        receipt=json.loads((root/'docs/commands/merge-faces-validation.json').read_text())
        self.assertEqual(receipt['native_command_cases'],77)
        self.assertEqual(receipt['registered_commands'],426)
        self.assertTrue(set(MERGE_COMMAND_MODULES)<=set(receipt['sources']))
        for name in MERGE_COMMAND_MODULES:
            self.assertTrue(set(MERGE_COMMAND_MODULES)<=set(with_shared_sources([name])),name)
        for name,digest in receipt['sources'].items():
            self.assertEqual(hashlib.sha256((root/name).read_bytes()).hexdigest(),digest,name)
        for family,hashes in receipt['families'].items():
            for folder,digest in hashes.items():
                self.assertEqual(hashlib.sha256((HERE/folder/(family+'.json')).read_bytes()).hexdigest(),digest)
