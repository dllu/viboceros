"""Fixed PlanarMesh captures, owned-session guards and complete qualification."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from . import planar_mesh_probe as probe
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations,stage_helpers
from .report_planar_mesh import HERE,ROOT,summarize


class PlanarMeshTests(unittest.TestCase):
    def test_native_postselection_controls_preserve_selection_release_and_sources(self):
        path=HERE/'fixtures/planar_mesh_postselect.json';q=json.loads(path.read_text());self.assertEqual(q,probe.post_request());validate_operations(q)
        r=json.loads((HERE/'observations/planar_mesh_postselect.json').read_text());self.assertEqual(len(r['results']),3)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);v=row['value'];self.assertEqual(v['input'],probe.POST_CASES[row['id']])
            self.assertTrue(v['sdk_inputs_unchanged']);self.assertTrue(all(not o['selected']for o in v['before']))
            self.assertTrue(all(not o['selected']for o in v['after_script']))
            event=[e for e in v['events']if e['name']=='PlanarMesh'];self.assertEqual(len(event),1)
            self.assertEqual(event[0]['result'],'Success'if row['id']=='post_rectangle'else'Failure')
            before={o['source_index']:o['crc']for o in v['before']}
            for phase in ['after_script','undo','redo']:
                for o in v[phase]:
                    if o['role']=='source':self.assertEqual(o['crc'],before[o['source_index']])

    def test_fixed_sources_and_immutable_native_provenance(self):
        path=HERE/'fixtures/planar_mesh.json';q=json.loads(path.read_text());self.assertEqual(q,probe.request());validate_operations(q)
        r=json.loads((HERE/'observations/planar_mesh.json').read_text());self.assertEqual(len(r['results']),21)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:
                data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value']
            self.assertEqual(v['input'],probe.CASES[row['id']]);self.assertTrue(v['sdk_inputs_unchanged'])
            self.assertEqual(len(v['sdk_single_boundaries']),len(v['input']['curves']))
            events=[e for e in v['events']if e['name']=='PlanarMesh'];self.assertEqual(len(events),1)
            self.assertIn(events[0]['result'],['Success','Failure'])
            originals={o['source_index']:o for o in v['before']}
            for phase in ['after_script','undo','redo']:
                for o in v[phase]:
                    if o['role']=='source':
                        original=originals[o['source_index']]
                        for key in ['crc','name','color','color_source','layer']:self.assertEqual(o[key],original[key])
                    else:
                        self.assertFalse(o['selected']);self.assertIsNone(o['name'])
                        self.assertEqual(o['layer'],'PlanarMesh output');self.assertEqual(o['color_source'],'ColorFromLayer')

    def test_only_fixed_private_bounded_one_iteration_probes_can_launch(self):
        for change in [{'case':[]},{'case':'arbitrary'},{'id':'arbitrary'},{'script':'arbitrary'}]:
            q=probe.request();q['operations'][0].update(change)
            with self.assertRaises(ValueError):validate_operations(q)
        with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
            client=OracleClient(settings_scheme='VibocerosOraclePlanarMeshTest20261021')
            for kind in ['repeat','boolean','count','duplicate']:
                q=probe.request()
                if kind=='repeat':q['iterations']=2
                elif kind=='boolean':q['iterations']=True
                elif kind=='count':q['operations']*=2
                else:q['operations'].append(copy.deepcopy(q['operations'][0]))
                with self.assertRaises(OracleProtocolError):client.run_rhino(q)
            spawn.assert_not_called()

    def test_helper_closure_is_complete(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(probe.request(),HERE,Path(directory))
            for name in ['planar_mesh_probe','join_probe','merge_edges_probe']:
                self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())

    def report(self):
        r=json.loads((ROOT/'docs/planar-mesh-qualification.json').read_text())
        lines={stage:['PLANAR_MESH_'+('COMMAND'if stage in ('native','portable')else stage.upper())+' '+json.dumps(row)
                      for row in r['stages'][stage]['cases']]for stage in r['stages']}
        lines['controls']=['PLANAR_MESH_POSTCONTROL '+json.dumps(row)for row in r['postselection_controls']['cases']]
        return r,lines

    def rebuild(self,lines):
        return summarize('\n'.join(lines['native']),'\n'.join(lines['portable']),
                         '\n'.join(lines['preselected']+lines['postselected']+lines['controls']))

    def test_reports_keep_geometry_and_topology_qualification_separate(self):
        r,lines=self.report();self.assertEqual(self.rebuild(lines),r)
        self.assertEqual(r['model_epsilon'],1e-6);self.assertEqual(r['recipes'],21)
        self.assertFalse(r['full_native_parity']);self.assertFalse(r['performance_qualified'])
        self.assertEqual(r['postselection_controls']['qualified_recipes'],3)
        gaps=['triangle','concave','tilted','circle','ellipse','nurbs_polyline','polygon_hole','hole_reverse','circle_hole','island','disconnected']
        for stage in r['stages'].values():
            self.assertEqual(stage['geometry_qualified_recipes'],21);self.assertEqual(stage['qualified_recipes'],10)
            self.assertEqual(stage['topology_gaps'],gaps);self.assertEqual(stage['outcome_gaps'],[])
        for path,sha in r['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)

    def test_reports_reject_missing_duplicate_nonfinite_and_hidden_topology_gaps(self):
        r,lines=self.report()
        for mutation in ['missing','duplicate','hidden','nonfinite','wrong_target','missing_source']:
            invalid=copy.deepcopy(lines)
            if mutation=='missing':invalid['native'].pop()
            elif mutation=='duplicate':invalid['portable'].append(invalid['portable'][0])
            else:
                row=copy.deepcopy(r['stages']['native']['cases'][0])
                if mutation=='hidden':row['qualified']=True;row['check']['counts_match']=True
                elif mutation=='nonfinite':row['check']['meshes'][0]['maximum_bidirectional_sample_error']=float('nan')
                elif mutation=='wrong_target':row['check']['meshes'][0]['native_index']=99
                else:row['check']['sources_and_attributes_preserved']=False
                invalid['native'][0]='PLANAR_MESH_COMMAND '+json.dumps(row)
            with self.assertRaises(ValueError):self.rebuild(invalid)


if __name__=='__main__':unittest.main()
