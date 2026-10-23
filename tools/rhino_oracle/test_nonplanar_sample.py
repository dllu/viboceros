"""Independent physical picks, threshold precision and retained incomplete attempts."""
from copy import deepcopy
import base64
import gzip
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .nonplanar_sample_cases import request,validate
from .report_nonplanar_sample import HERE,ROOT,FAMILIES,native,summarize
from .structural_sdk import validate_operations,stage_helpers


class NonplanarSampleTests(unittest.TestCase):
    def test_inputs_physical_clicks_exact_archives_and_frozen_producers(self):
        version,rows=native();self.assertEqual(len(rows),25)
        for family,(flag,count)in FAMILIES.items():
            query=HERE/'fixtures'/(family+'.json');data=json.loads((HERE/'observations'/(family+'.json')).read_text());payload=request(**{flag:True})
            self.assertEqual(json.loads(query.read_text()),payload);validate_operations(payload)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))
        self.assertEqual(sum(picks for v,pref,picks in rows.values()),20)

    def test_sampling_changes_one_value_and_uses_decimal_steps(self):
        rows=native()[1]
        for name,distance in [('shaded_distance',.19508001458970664),('scaled_large',1.9508001458970665),('scaled_small',.00019508001458970662),('power_one',.999),('power_tenth',.0999)]:
            self.assertAlmostEqual(rows['nonplanar_sample_'+name][1]['distance'],distance,places=14)
        self.assertEqual(rows['nonplanar_sample_distance_switch_main'][1]['angle'],7.)
        self.assertEqual(rows['nonplanar_sample_angle_switch_main'][1]['distance'],.123)
        self.assertEqual(rows['nonplanar_sample_power_below'][0]['input']['sources'][0]['vertices'][0][2],math.nextafter(.5,0.))
        self.assertEqual(rows['nonplanar_sample_power_above'][0]['input']['sources'][0]['vertices'][0][2],math.nextafter(.5,1.))
        self.assertGreater(rows['nonplanar_sample_power_below'][1]['distance'],rows['nonplanar_sample_power_one'][1]['distance'])
        self.assertLess(rows['nonplanar_sample_small_angle'][1]['angle'],.01)

    def test_flat_sampled_negatives_persist_but_typed_negatives_clamp(self):
        rows=native()[1]
        for name,key in [('flat_distance','distance'),('flat_angle','angle')]:
            value,pref,picks=rows['nonplanar_sample_'+name];self.assertEqual(pref[key],-.01)
            self.assertEqual(next(r['mesh']['faces']for r in value['before']if r['source']==0),next(r['mesh']['faces']for r in value['after']if r['source']==0))
        self.assertEqual(rows['nonplanar_sample_after_cancel'][1]['angle'],-.01)
        for name,key in [('negative_distance','distance'),('negative_angle','angle')]:self.assertEqual(rows['nonplanar_sample_'+name][1][key],0.)
        for name in ['cancel_face','after_cancel','enter_keeps']:
            self.assertEqual(rows['nonplanar_sample_'+name][2],0)
        self.assertFalse(rows['nonplanar_sample_cancel_face'][0]['success'])

    def test_document_tolerances_external_reference_and_second_face(self):
        rows=native()[1]
        for name in ['tolerance_low','tolerance_high']:
            self.assertEqual(rows['nonplanar_sample_'+name][1]['distance'],rows['nonplanar_sample_shaded_distance'][1]['distance'])
        self.assertEqual(rows['nonplanar_sample_angular_tolerance'][1]['angle'],rows['nonplanar_sample_shaded_angle'][1]['angle'])
        value=rows['nonplanar_sample_external_reference'][0]
        self.assertFalse(next(r['selected']for r in value['before']if r['source']==1))
        self.assertEqual(next(r['mesh']for r in value['before']if r['source']==1),next(r['mesh']for r in value['after']if r['source']==1))
        self.assertEqual(rows['nonplanar_sample_raw_second'][0]['input']['sample_face'],1)

    def test_allowlist_standalone_import_and_immutable_recapture(self):
        payload=request(core=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with self.assertRaises(ValueError):request(core=True,shaded=True)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import nonplanar_sample_probe',directory],check=True,capture_output=True)
        for family,(flag,count)in FAMILIES.items():
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_nonplanar_sample','--'+flag.replace('_','-')],cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr);self.assertEqual([p.read_bytes()for p in paths],before)

    def test_incomplete_and_failed_attempts_qualify_no_geometry(self):
        data=json.loads((HERE/'observations/nonplanar_sample_discovery.json').read_text());self.assertEqual(len(data['results']),4)
        self.assertTrue(all(r['value']['input_trace'][0]['errors']for r in data['results']))
        for name,query,count in [('main_timeout','main',16),('main_interrupted','main_v2',16),('quad_ngon_timeout','quad_ngon',1)]:
            data=json.loads((HERE/'observations'/('nonplanar_sample_'+name+'.json')).read_text())
            self.assertEqual((data['requested_workflows'],data['qualified_workflows']),(count,0));self.assertFalse(data['response_retained'])
            self.assertEqual(data['request_sha256'],hashlib.sha256((HERE/'fixtures'/('nonplanar_sample_'+query+'.json')).read_bytes()).hexdigest())
            self.assertEqual(data['log_sha256'],hashlib.sha256((HERE/'observations'/('nonplanar_sample_'+name+'.log')).read_bytes()).hexdigest())
            if 'progress_sha256'in data:self.assertEqual(data['progress_sha256'],hashlib.sha256((HERE/'observations/nonplanar_sample_main_interrupted_progress.log').read_bytes()).hexdigest())
            for digest in data['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            self.assertFalse((HERE/'observations'/('nonplanar_sample_'+query+'.json')).exists())

    def evidence(self):
        data=json.loads((ROOT/'docs/nonplanar-sample-qualification.json').read_text());return data,['FACE_SAMPLER '+json.dumps(r)for r in data['cases']]

    def test_report_reconstructs_with_explicit_limits(self):
        data,lines=self.evidence();self.assertEqual(summarize('\n'.join(reversed(lines))),data)
        self.assertEqual((data['native_workflows'],data['face_samples'],data['accepted_commands']),(25,20,24))
        self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified']);self.assertEqual(data['failed_batch_qualified_workflows'],0)

    def test_missing_duplicate_and_forged_scalar_or_pick_evidence_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for row in [data['cases'][0],next(r for r in data['cases']if r['face_picks']==0)]:
            remaining=[s for s in lines if s!='FACE_SAMPLER '+json.dumps(row)]
            edits=[(k,False)for k in ['qualified','geometry','channels','identity','metadata','selection','object_order','history','staged']]
            edits += [('face_picks',True),('raw_face_picker',not row['raw_face_picker']),('accepted',not row['accepted']),('maximum_error',float('nan')),('maximum_distance_error',1e-5),('maximum_angle_error_degrees',1e-5),('preferences',dict(row['preferences'],angle=True))]
            for key,value in edits:
                bad=deepcopy(row);bad[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+['FACE_SAMPLER '+json.dumps(bad)]))
if __name__=='__main__':unittest.main()
