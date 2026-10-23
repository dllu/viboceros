"""Independent SDK inputs, frozen capture producers and strict lifecycle reports."""
from copy import deepcopy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .quad_conversion_cases import request, validate
from .report_quad_conversion import HERE, ROOT, native, summarize
from .structural_sdk import stage_helpers, validate_operations


class QuadConversionTests(unittest.TestCase):
    def test_references_bind_independent_recipes_and_frozen_producers(self):
        for family in ['mesh_quad_conversion_v2','mesh_quad_conversion_v3']:
            query=HERE/'fixtures'/(family+'.json')
            self.assertEqual(json.loads(query.read_text()),request())
            data=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            self.assertEqual(len(data['results']),14)
            for digest in data['provenance']['sources'].values():
                frozen=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)
        complete=native()
        earlier=json.loads((HERE/'observations/mesh_quad_conversion_v2.json').read_text())
        for old,new in zip(earlier['results'],complete['results']):
            self.assertEqual(old['id'],new['id'])
            for stage in ['before','after']:
                self.assertEqual(set(old['value'][stage]),{'vertices','faces'})
                for key in ['vertices','faces']:
                    self.assertEqual(old['value'][stage][key],new['value'][stage][key])
            self.assertEqual(old['value']['changed'],new['value']['changed'])

    def test_invalid_native_overlay_is_explicit_despite_mesh_is_valid(self):
        value=next(r['value']for r in native()['results']if r['id']=='quad_convert_adjacent_ngons')
        self.assertIs(value['valid_after'],True)
        self.assertEqual(value['after']['faces'],[[2,3,0,1]])
        self.assertEqual(value['after']['ngons'],[dict(vertices=[0,1,2],faces=[0])])
        self.assertNotEqual(set(value['after']['ngons'][0]['vertices']),set(value['after']['faces'][0]))

    def test_mutual_longest_edges_and_angle_reset_have_independent_witnesses(self):
        rows={r['id']:r['value']for r in native()['results']}
        for key in ['short_two','square_extra','warp_accept','channels_ngon']:
            self.assertIs(rows['quad_convert_'+key]['changed'],True)
        for key in ['short_extra','tied_fan','warp_reject','angle_reset','raw_seam','opposed']:
            self.assertIs(rows['quad_convert_'+key]['changed'],False)
        self.assertEqual(rows['quad_convert_unused_vertex']['before']['vertices'],rows['quad_convert_unused_vertex']['after']['vertices'])

    def test_recipe_allowlist_and_standalone_worker_bundle(self):
        query=request();validate_operations(query)
        for invalid in [None,[],{},dict(query['operations'][0],path='arbitrary'),dict(query['operations'][0],case='arbitrary')]:
            with self.assertRaises(ValueError):validate(invalid)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import quad_conversion_probe',directory],check=True,capture_output=True)
        paths=[HERE/'fixtures/mesh_quad_conversion_v3.json',HERE/'observations/mesh_quad_conversion_v3.json']
        before=[p.read_bytes()for p in paths]
        result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_quad_conversion'],cwd=ROOT,text=True,capture_output=True)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('observations are immutable',result.stderr)
        self.assertEqual([p.read_bytes()for p in paths],before)

    def test_dispatch_failure_is_preserved_without_qualified_geometry(self):
        value=json.loads((HERE/'observations/mesh_quad_conversion_dispatch_failure.json').read_text())
        self.assertEqual(value['status'],'worker_dispatch_failure')
        self.assertEqual(value['qualified_workflows'],0)
        self.assertIs(value['response_retained'],False)
        for key,name in [('request_sha256','fixtures/mesh_quad_conversion.json'),('log_sha256','observations/mesh_quad_conversion_dispatch_failure.log')]:
            self.assertEqual(value[key],hashlib.sha256((HERE/name).read_bytes()).hexdigest())
        for digest in value['sources'].values():
            self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
        self.assertFalse((HERE/'observations/mesh_quad_conversion.json').exists())

    def evidence(self):
        report=json.loads((ROOT/'docs/quad-conversion-qualification.json').read_text())
        return report,['QUAD_CONVERSION '+json.dumps(row)for row in report['cases']]

    def test_report_reconstructs_without_claiming_native_prompt_parity(self):
        report,lines=self.evidence()
        self.assertEqual(summarize('\n'.join(reversed(lines))),report)
        self.assertEqual(report['sdk_geometry_cases'],14)
        self.assertEqual(report['local_command_cases'],14)
        self.assertFalse(report['native_command_prompt_qualified'])
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])

    def test_missing_duplicate_nonfinite_and_relabelled_replays_fail(self):
        report,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        row=report['cases'][0]
        for key,value in [('qualified',False),('history',False),('metadata',False),('channels',False),('selection',False),('raw_faces',False),('changed',False),('command_syntaxes',1),('pairs',True),('maximum_error',float('nan')),('maximum_error',1e-3)]:
            bad=deepcopy(row);bad[key]=value
            with self.assertRaises(ValueError):summarize('\n'.join(['QUAD_CONVERSION '+json.dumps(bad)]+lines[1:]))
        gap=next(r for r in report['cases']if r['id']=='quad_convert_adjacent_ngons')
        remaining=[s for s in lines if s!='QUAD_CONVERSION '+json.dumps(gap)]
        for key,value in [('native_invalid_ngons',[]),('native_ngons_correspond',True),('ngons',1)]:
            bad=deepcopy(gap);bad[key]=value
            with self.assertRaises(ValueError):summarize('\n'.join(remaining+['QUAD_CONVERSION '+json.dumps(bad)]))


if __name__=='__main__':unittest.main()
