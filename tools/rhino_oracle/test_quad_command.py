"""Independent command sources, immutable captures and strict qualification."""
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
from .quad_command_cases import request, validate
from .report_quad_command import FAMILIES, HERE, ROOT, native, summarize
from .structural_sdk import validate_operations, stage_helpers


class QuadCommandTests(unittest.TestCase):
    def test_fixed_inputs_frozen_producers_and_original_source_files(self):
        self.assertEqual(len(native()),20)
        for family in FAMILIES:
            query=HERE/'fixtures'/(family+'.json');payload=request(main=family.endswith('main'))
            self.assertEqual(json.loads(query.read_text()),payload);validate_operations(payload)
            data=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_recapture_is_rejected_without_mutating_evidence(self):
        for family in FAMILIES:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_quad_command']+(['--main']if family.endswith('main')else[]),cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr)
            self.assertEqual([p.read_bytes()for p in paths],before)

    def test_allowlist_and_standalone_worker(self):
        payload=request(main=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import quad_command_probe',directory],check=True,capture_output=True)

    def test_native_defaults_and_cancellation_are_distinct_from_sdk_no_ops(self):
        rows=native()
        self.assertEqual(rows['quad_command_fresh_defaults'][1],dict(planarity=5.,rectangularity=2.))
        self.assertEqual(rows['quad_command_remembered_low'][1]['planarity'],1.)
        self.assertEqual(rows['quad_command_cancel_options'][1],dict(planarity=5.,rectangularity=3.))
        self.assertEqual(rows['quad_command_after_cancel'][1],dict(planarity=5.,rectangularity=3.))
        for key in ['quad_command_low_angle','quad_command_default_warp','quad_command_raw_seam']:
            value=rows[key][0]
            self.assertEqual(next(o['mesh']['faces']for o in value['before']if o['source']==0),next(o['mesh']['faces']for o in value['after']if o['source']==0))
            self.assertTrue(any(o['name']=='Undo witness'for o in value['undo']))
            self.assertTrue(all(not o['selected']for o in value['after']))
        for key in ['quad_command_cancel_options','quad_command_cancel_selection']:
            self.assertFalse(any(o['name']=='Undo witness'for o in rows[key][0]['undo']))

    def evidence(self):
        data=json.loads((ROOT/'docs/quad-command-qualification.json').read_text())
        return data,['QUAD_'+stage.upper()+' '+json.dumps(r)for stage in ['command','getter']for r in data[stage]]

    def test_report_reconstructs_all_cases_with_explicit_limits(self):
        data,lines=self.evidence();self.assertEqual(summarize('\n'.join(reversed(lines))),data)
        self.assertEqual((data['command_cases'],data['getter_cases']),(18,20))
        self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified'])

    def test_missing_duplicate_and_forged_workflows_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for stage in ['command','getter']:
            row=data[stage][0];needle='QUAD_'+stage.upper()+' '+json.dumps(row)
            remaining=[s for s in lines if s!=needle]
            for key,value in [('qualified',False),('geometry',False),('channels',False),('identity',False),('metadata',False),('object_order',False),('selection',False),('history',False),('accepted',False),('preferences',dict(planarity=1.,rectangularity=2.)),('preferences',dict(planarity=5.,rectangularity=True)),('maximum_error',float('nan')),('maximum_error',1e-3)]:
                bad=deepcopy(row);bad[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+['QUAD_'+stage.upper()+' '+json.dumps(bad)]))
if __name__=='__main__':unittest.main()
