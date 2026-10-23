"""Independent command inputs, retained discovery failures and strict admission."""
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
from .nonplanar_command_cases import request,validate
from .report_nonplanar_command import FAMILIES,EXCLUDED,HERE,ROOT,native,summarize
from .structural_sdk import validate_operations,stage_helpers


def flags(family):
    return dict(options=family.endswith('options'),syntax=family.endswith('syntax_v2'),main=family.endswith('main'),selection=family.endswith('selection'))


class NonplanarCommandTests(unittest.TestCase):
    def test_fixed_requests_frozen_producers_and_exact_source_archives(self):
        self.assertEqual(len(native()[1]),35)
        for family in FAMILIES:
            query=HERE/'fixtures'/(family+'.json');payload=request(**flags(family));self.assertEqual(json.loads(query.read_text()),payload);validate_operations(payload)
            data=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_recapture_preserves_all_existing_evidence(self):
        for family in FAMILIES:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_nonplanar_command']+['--'+k for k,v in flags(family).items()if v],cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr)
            self.assertEqual([p.read_bytes()for p in paths],before)

    def test_allowlist_and_standalone_worker(self):
        payload=request(main=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import nonplanar_command_probe',directory],check=True,capture_output=True)

    def test_native_defaults_option_memory_no_ops_and_cancellation(self):
        rows=native()[1]
        self.assertEqual(rows['nonplanar_command_main_defaults'][1],dict(select_by='Distance',distance=0.,angle=0.,split_method='ShortestDiagonal'))
        self.assertEqual(rows['nonplanar_command_remembered_high'][1]['distance'],.25)
        self.assertEqual(rows['nonplanar_command_remembered_angle'][1],dict(select_by='Angle',distance=.01,angle=30.,split_method='MinimumArea'))
        self.assertEqual(rows['nonplanar_command_after_cancel'][1],dict(select_by='Angle',distance=.01,angle=0.,split_method='ShortestDiagonal'))
        self.assertEqual(rows['nonplanar_command_toggle'][1]['select_by'],'Distance')
        self.assertEqual(rows['nonplanar_command_postselect_high'][1]['distance'],.25)
        self.assertEqual(rows['nonplanar_command_postselect_low'][1]['distance'],.01)
        for key in ['nonplanar_command_default_flat','nonplanar_command_default_triangles','nonplanar_command_distance_high']:
            value=rows[key][0]
            self.assertEqual(next(s['mesh']['faces']for s in value['before']if s['source']==0),next(s['mesh']['faces']for s in value['after']if s['source']==0))
            self.assertTrue(any(s['name']=='Undo witness'for s in value['undo']))
            self.assertTrue(all(not s['selected']for s in value['after']))
        for key in ['nonplanar_command_cancel_options','nonplanar_command_cancel_selection','nonplanar_command_selection_cancel','nonplanar_command_split_menu']:
            self.assertFalse(any(s['name']=='Undo witness'for s in rows[key][0]['undo']))
        self.assertEqual(len(EXCLUDED),5)
        for key in EXCLUDED:self.assertIn('Unknown command',rows[key][0]['history'])

    def test_failed_import_retains_zero_qualified_geometry(self):
        data=json.loads((HERE/'observations/nonplanar_command_import_failure.json').read_text())
        self.assertEqual(data['status'],'worker_import_failure');self.assertEqual((data['requested_workflows'],data['qualified_workflows']),(6,0));self.assertFalse(data['response_retained'])
        for key,path in [('request_sha256','fixtures/nonplanar_command_syntax.json'),('log_sha256','observations/nonplanar_command_import_failure.log')]:
            self.assertEqual(data[key],hashlib.sha256((HERE/path).read_bytes()).hexdigest())
        for digest in data['sources'].values():
            self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
        self.assertFalse((HERE/'observations/nonplanar_command_syntax.json').exists())

    def evidence(self):
        data=json.loads((ROOT/'docs/nonplanar-command-qualification.json').read_text())
        return data,['NONPLANAR_'+stage.upper()+' '+json.dumps(row)for stage in ['command','getter']for row in data[stage]]

    def test_report_reconstructs_with_explicit_exclusions_and_limits(self):
        data,lines=self.evidence();self.assertEqual(summarize('\n'.join(reversed(lines))),data)
        self.assertEqual((data['captured_workflows'],data['native_workflows'],data['command_cases'],data['getter_cases']),(35,30,26,30))
        self.assertEqual(data['excluded'],EXCLUDED);self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified'])

    def test_missing_duplicate_forged_and_nonfinite_replays_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for stage in ['command','getter']:
            row=data[stage][0];remaining=[s for s in lines if s!='NONPLANAR_'+stage.upper()+' '+json.dumps(row)]
            edits=[(k,False)for k in ['qualified','geometry','channels','identity','metadata','object_order','selection','history','accepted']]
            edits += [('maximum_error',float('nan')),('maximum_error',1e-3),('id',next(iter(EXCLUDED))),('preferences',dict(row['preferences'],distance=True)),('preferences',dict(row['preferences'],angle=30.))]
            for key,value in edits:
                bad=deepcopy(row);bad[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+['NONPLANAR_'+stage.upper()+' '+json.dumps(bad)]))
if __name__=='__main__':unittest.main()
