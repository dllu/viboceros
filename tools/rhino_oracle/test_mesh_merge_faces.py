"""Actual MergeFaces storage and bounded, immutable native capture producers."""
import copy
import gzip
import hashlib
import json
import os
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock
from .client import OracleClient, OracleProtocolError, _compare_value
from .generate_mesh_merge_faces import request
from .generate_mesh_merge_faces_boundaries import request as boundaries
from .mesh_merge_faces_probe import validate
from .qualification_sources import with_shared_sources
from .structural_sdk import stage_helpers, validate_operations

HERE=Path(__file__).resolve().parent
FAMILIES=[('mesh_merge_faces',request,36),('mesh_merge_faces_boundaries',boundaries,12)]
def rows(family):
    return {r['id']:r['value']for r in json.loads((HERE/'observations'/(family+'.json')).read_text())['results']}

class MeshMergeFacesTests(unittest.TestCase):
    def test_complete_recipes_results_and_content_addressed_provenance(self):
        outcomes=Counter();edits=0
        for family,make,count in FAMILIES:
            q=make();validate_operations(q)
            path=HERE/'fixtures'/(family+'.json');self.assertEqual(json.loads(path.read_text()),q)
            r=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(r['results']),count)
            self.assertEqual([o['id']for o in q['operations']],[row['id']for row in r['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name,digest in r['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=digest:
                    data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            for row in r['results']:
                v=row['value'];outcomes[v['command_result']]+=1;edits+=v['before']!=v['after']
                self.assertTrue(v['attributes_preserved']);self.assertTrue(v['before']['valid']);self.assertTrue(v['after']['valid'])
                self.assertEqual(v['undone'],v['before']);self.assertEqual(v['redone'],v['after'])
        self.assertEqual(outcomes,dict(Success=46,Cancel=2));self.assertEqual(edits,36)
    def test_holes_and_existing_logical_faces_are_distinct(self):
        base=rows('mesh_merge_faces');holdout=rows('mesh_merge_faces_boundaries')
        for channels in ['none','all']:
            ring=base['ring-'+channels];self.assertEqual(ring['before'],ring['after'])
            old=holdout['existing-hole-'+channels];self.assertEqual(old['before'],old['after']);self.assertEqual(len(old['after']['ngons']),1)
            mixed=holdout['hole-and-pair-'+channels];self.assertEqual(len(mixed['after']['ngons']),1)
            self.assertEqual(len(mixed['after']['faces']),9)
            partial=holdout['partial-ring-'+channels];self.assertEqual(len(partial['after']['ngons']),1)
            reverse=holdout['reversed-ring-'+channels];self.assertEqual(reverse['before'],reverse['after'])
            edge=holdout['edge-ring-'+channels];self.assertEqual(edge['before'],edge['after'])
    def test_explicit_face_selection_can_weld_a_seam_and_retain_all_channels(self):
        value=rows('mesh_merge_faces')['unwelded-all']
        self.assertEqual(len(value['before']['vertices']),8);self.assertEqual(len(value['after']['vertices']),6)
        for name in ['colors','uv','normals']:self.assertEqual(len(value['after'][name]),6)
        self.assertEqual(len(value['after']['ngons']),1)
        for name in ['strip-faces-all','bent-faces-all']:
            v=rows('mesh_merge_faces')[name]
            for field in ['vertices','colors','uv','normals']:self.assertEqual(v['before'][field],v['after'][field])
    def test_native_defaults_select_results_but_do_not_select_the_whole_owner(self):
        for family,_,_ in FAMILIES:
            for v in rows(family).values():
                self.assertFalse(v['selected_owner'])
                if v['before']!=v['after']:self.assertTrue(v['selected_after'])
        for name,v in rows('mesh_merge_faces').items():
            if name.startswith('empty-selection'):
                self.assertEqual(v['command_result'],'Cancel')
                # The saved flag measures bounded timer input, not a getter
                # that may open and close while the macro consumes Enter.
                self.assertFalse(v['getter_opened'])
    def test_closed_recipes_and_helpers(self):
        q=request()
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},{'mesh_merge_faces_probe.py','hide_pt_input.py','join_probe.py','merge_edges_probe.py'})
        for value in [None,[],True,'MergeFaces']:
            with self.assertRaises(ValueError):validate(value)
        for patch in [dict(id='bad _Delete'),dict(components=[dict(kind='face',index=True)]),dict(components=[dict(kind='vertex',index=128)]),dict(components=[dict(kind='edge',vertices=[0,128])]),dict(components=[dict(kind='face',index=0,extra=True)]),dict(vertices=[[float('nan'),0,0]]*4),dict(faces=[[0,1,[]]]),dict(ngons=[dict(vertices=[0,1,2],faces=[999])]),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(copy.deepcopy(q['operations'][0]),**patch))
    def test_private_dedicated_single_iteration_capture_preflight(self):
        q=request();mixed=dict(q,operations=q['operations']+[dict(op='curve_length',id='unrelated')])
        for scheme,query in [(None,q),('VibocerosOracleMergeTest',dict(q,iterations=True)),('VibocerosOracleMergeTest',dict(q,iterations=2)),('VibocerosOracleMergeTest',dict(q,operations=q['operations']*2)),('VibocerosOracleMergeTest',mixed)]:
            with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                  self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(query)
            launch.assert_not_called()
    def test_native_body_qualification_covers_rust_cpp_and_boundary_policy(self):
        bodies=['crates/viboceros-geometry/tests/support/mesh_face_merge.rs',
                'crates/viboceros-geometry/tests/mesh_face_merge.rs',
                'crates/viboceros-oracle/tests/mesh_face_merge.rs',
                'crates/viboceros-geometry/src/mesh/merge_faces.rs',
                'crates/viboceros-geometry/src/mesh/merge_faces/tests.rs',
                'crates/viboceros-opennurbs/src/mesh/merge.rs',
                'crates/viboceros-opennurbs/native/mesh_merge.h',
                'crates/viboceros-opennurbs/native/mesh_merge_selection.h',
                'third_party/patches/opennurbs-merge-face-holes.cmake']
        for source in bodies+['crates/viboceros-geometry/src/mesh.rs','crates/viboceros-opennurbs/src/mesh.rs']:
            expanded=with_shared_sources([source]);self.assertTrue(set(bodies)<=set(expanded),source)
            self.assertIn('crates/viboceros-opennurbs/build.rs',expanded)
    def test_complete_storage_comparison_detects_channels_faces_and_ngon_membership(self):
        original=rows('mesh_merge_faces')['strip-faces-all']['after']
        for name in ['vertices','faces','colors','uv','normals','ngons']:
            changed=copy.deepcopy(original)
            if name=='ngons':changed[name][0]['faces']=[0]
            else:changed[name][0][0]+=0.01 if name in ['vertices','uv','normals']else 1
            differences=[];_compare_value('mesh',original,changed,1e-7,1e-12,differences)
            self.assertTrue(differences,name)
