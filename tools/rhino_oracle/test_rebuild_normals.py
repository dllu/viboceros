"""Normal rebuilding captures, native compaction and strict report admission."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .rebuild_normals_cases import request,validate
from .report_rebuild_normals import HERE,ROOT,FAMILIES,native,summarize
from .structural_sdk import validate_operations,stage_helpers


class RebuildNormalsTests(unittest.TestCase):
    def test_fixed_inputs_frozen_producers_and_exact_command_sources(self):
        version,groups=native();self.assertEqual((len(groups['sdk']),len(groups['command'])),(19,22))
        for family,count in FAMILIES.items():
            query=HERE/'fixtures'/(family+'.json');payload=request(commands=family.endswith('command'),holdouts=family.endswith('holdouts'));validate_operations(payload)
            data=json.loads((HERE/'observations'/(family+'.json')).read_text());self.assertEqual(json.loads(query.read_text()),payload);self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                if 'command'in row['value']['input']:self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_compaction_remaps_channels_and_ngons_without_merging_seams(self):
        groups=native()[1];v=groups['sdk']['rebuild_normals_holdout_channels']
        self.assertEqual((len(v['before']['vertices']),len(v['after']['vertices'])),(7,4))
        self.assertEqual(v['after']['faces'],[[0,1,2,3]]);self.assertEqual(v['after']['ngons'],[dict(vertices=[0,1,2,3],faces=[0])])
        self.assertEqual(v['after']['uv'],[v['before']['uv'][i]for i in [1,2,4,5]]);self.assertEqual(v['after']['colors'],[v['before']['colors'][i]for i in [1,2,4,5]])
        seam=groups['sdk']['rebuild_normals_holdout_seams']['after'];self.assertEqual(len(seam['vertices']),6);self.assertEqual(seam['faces'],[[0,1,2],[3,4,5]])
        self.assertNotEqual(seam['normals'][0],seam['normals'][4])

    def test_normal_repair_and_native_document_admission_are_distinct(self):
        groups=native()[1]
        for name in ['rebuild_normals_zero','rebuild_normals_holdout_nonunit']:
            v=groups['sdk'][name];self.assertFalse(v['before']['valid']);self.assertTrue(v['after']['valid'])
        for name in ['rebuild_normals_triangle','rebuild_normals_fold']:
            self.assertEqual(groups['sdk'][name]['before']['normals'],[])
        self.assertEqual(groups['sdk']['rebuild_normals_collapsed']['after']['face_normals'][0],[0.,0.,0.])
        self.assertEqual(groups['sdk']['rebuild_normals_opposed']['after']['normals'],[[0.,0.,1.]]*3)
        noop=groups['command']['rebuild_normals_holdout_command_noop'];self.assertEqual(noop['before'][0]['mesh'],noop['after'][-1]['mesh']);self.assertEqual([r['source']for r in noop['undo']],[0,None]);self.assertFalse(noop['redo'][-1]['selected'])
        self.assertEqual(len(groups['command']['rebuild_normals_command_quad']['before'][0]['mesh']['vertex_normals']),4)

    def test_allowlist_standalone_staging_and_immutable_recapture(self):
        payload=request(commands=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory));subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import rebuild_normals_probe',directory],check=True,capture_output=True)
        for family,flags in [('rebuild_normals_sdk',[]),('rebuild_normals_command',['--commands']),('rebuild_normals_holdouts',['--holdouts'])]:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_rebuild_normals']+flags,cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr);self.assertEqual([p.read_bytes()for p in paths],before)

    def evidence(self):
        data=json.loads((ROOT/'docs/rebuild-normals-qualification.json').read_text())
        return data,['REBUILD_NORMALS_'+stage.upper()+' '+json.dumps(row)for stage in ['sdk','command','getter']for row in data[stage]]

    def test_report_reconstructs_with_explicit_normal_query_limits(self):
        data,lines=self.evidence();self.assertEqual(reconstruct_measurement(data, summarize, '\n'.join(reversed(lines))),data)
        self.assertEqual((data['sdk_cases'],data['native_command_workflows'],data['registered_command_cases'],data['getter_cases']),(19,22,20,22))
        self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified']);self.assertEqual(sum(r['collapsed_face_queries']for r in data['sdk']),1)

    def test_missing_duplicate_and_forged_evidence_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for stage in ['sdk','command','getter']:
            original=data[stage][0];prefix='REBUILD_NORMALS_'+stage.upper()+' ';remaining=[s for s in lines if s!=prefix+json.dumps(original)]
            edits=[('qualified',False),('channels',False),('normals',False),('ngons',False),('maximum_error',float('nan'))]
            edits += [('vertices',True),('idempotent',False),('maximum_face_error',True),('maximum_face_error',1e-5),('collapsed_face_queries',True)]if stage=='sdk'else[('accepted',False),('selection',False),('object_order',False),('history',False)]
            for key,value in edits:
                row=deepcopy(original);row[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+[prefix+json.dumps(row)]))
if __name__=='__main__':unittest.main()
