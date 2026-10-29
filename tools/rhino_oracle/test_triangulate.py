"""Frozen SDK/command distinctions and strict independent replay evidence."""
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
from .triangulate_cases import request,validate
from .report_triangulate import HERE,ROOT,FAMILIES,native,summarize
from .structural_sdk import validate_operations,stage_helpers


class TriangulateTests(unittest.TestCase):
    def test_exact_recipes_source_archives_and_frozen_producers(self):
        version,groups=native();self.assertEqual((len(groups['sdk']),len(groups['command'])),(17,19))
        for family,(kind,count)in FAMILIES.items():
            path=HERE/'fixtures'/(family+'.json');payload=request(commands=family=='triangulate_command',holdouts=family=='triangulate_holdouts');validate_operations(payload)
            self.assertEqual(json.loads(path.read_text()),payload)
            data=json.loads((HERE/'observations'/(family+'.json')).read_text());self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                if kind=='command':self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_sdk_ngon_membership_and_command_overlay_removal_differ(self):
        groups=native()[1];v=groups['sdk']['triangulate_ngon_two_quads']
        self.assertEqual(v['before']['ngons'][0]['faces'],[1,0]);self.assertEqual(v['after']['ngons'][0]['faces'],[1,0,2,3])
        self.assertEqual(groups['sdk']['triangulate_two_ngons']['after']['ngons'][0]['faces'],[1,3])
        for name in ['triangulate_command_ngon','triangulate_command_ngon_two_quads','triangulate_holdout_triangle_ngon','triangulate_holdout_partial_ngons','triangulate_holdout_reversed_ngons']:
            v=groups['command'][name];self.assertTrue(v['before'][0]['mesh']['ngons'])
            self.assertEqual(next(o['mesh']['ngons']for o in v['after']if o['source']==0),[])

    def test_native_selection_order_cancellation_and_noop_history(self):
        rows=native()[1]['command']
        for name in ['triangulate_command_square','triangulate_command_triangles','triangulate_holdout_triangle_ngon']:
            v=rows[name];self.assertEqual([r['source']for r in v['after']],[None,0]);self.assertTrue(v['after'][-1]['selected']);self.assertTrue(v['undo'][0]['selected']);self.assertFalse(v['redo'][-1]['selected'])
        for name in ['triangulate_command_postselect','triangulate_holdout_postselect_triangles']:
            self.assertFalse(rows[name]['after'][-1]['selected']);self.assertEqual(len(rows[name]['undo']),2)
        for name in ['triangulate_command_cancel','triangulate_holdout_cancel_picks']:
            self.assertFalse(rows[name]['success']);self.assertEqual(rows[name]['before'],rows[name]['after']);self.assertEqual(len(rows[name]['undo']),1)
        self.assertEqual([r['source']for r in rows['triangulate_command_two_meshes']['after']],[None,0,1])

    def test_allowlist_standalone_staging_and_immutable_recaptures(self):
        payload=request(commands=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory));subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import triangulate_probe',directory],check=True,capture_output=True)
        for family,flags in [('triangulate_sdk',[]),('triangulate_command',['--commands']),('triangulate_holdouts',['--holdouts'])]:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            p=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_triangulate']+flags,cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(p.returncode,0);self.assertIn('observations are immutable',p.stderr);self.assertEqual([p.read_bytes()for p in paths],before)

    def evidence(self):
        data=json.loads((ROOT/'docs/triangulate-qualification.json').read_text())
        return data,['TRIANGULATE_'+stage.upper()+' '+json.dumps(row)for stage in ['sdk','command','getter']for row in data[stage]]

    def test_report_reconstructs_and_retains_scope(self):
        data,lines=self.evidence();self.assertEqual(reconstruct_measurement(data, summarize, '\n'.join(reversed(lines))),data)
        self.assertEqual((data['sdk_cases'],data['native_command_workflows'],data['registered_command_cases'],data['getter_cases']),(17,19,17,19))
        self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified'])

    def test_missing_duplicate_or_forged_evidence_fails(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for stage in ['sdk','command','getter']:
            original=data[stage][0];prefix='TRIANGULATE_'+stage.upper()+' ';remaining=[s for s in lines if s!=prefix+json.dumps(original)]
            edits=[('qualified',False),('channels',False),('ngons',False),('maximum_error',float('nan'))]
            edits += [('converted',True),('raw_faces',False),('tolerance_independent',False)]if stage=='sdk'else[('accepted',False),('selection',False),('object_order',False),('history',False)]
            for key,value in edits:
                row=deepcopy(original);row[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+[prefix+json.dumps(row)]))
if __name__=='__main__':unittest.main()
