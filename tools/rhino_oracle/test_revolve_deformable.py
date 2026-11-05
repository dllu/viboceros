"""Frozen native geometry, source policy, domain rules and isolated staging."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .revolve_deformable_cases import cases, pilot_cases, holdout_cases, domain_cases, threshold_cases, scale_cases, policy_cases, upper_cases
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

COMMANDS = {
    'revolve_deformable_pilot': pilot_cases,
    'revolve_deformable': cases,
    'revolve_deformable_holdouts': holdout_cases,
    'revolve_deformable_domains': domain_cases,
    'revolve_deformable_thresholds': threshold_cases,
    'revolve_deformable_scales': scale_cases,
    'revolve_deformable_policy': policy_cases,
    'revolve_deformable_upper': upper_cases,
}
SDK = ['revolve_parameterization','revolve_domain_sampling','revolve_domain_lengths','revolve_domain_estimates']


class DeformableRevolveTests(unittest.TestCase):
    def test_requests_and_frozen_native_producers(self):
        for family in list(COMMANDS)+SDK:
            path=HERE/('fixtures/'+family+'.json');query=json.loads(path.read_bytes());data=read(family)
            if family in COMMANDS:self.assertEqual(query['operations'],COMMANDS[family]())
            validate_operations(query)
            self.assertEqual([row['id']for row in data['results']],[op['id']for op in query['operations']])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)

    def test_public_commands_preserve_sources_selection_and_history(self):
        total=0
        for family,make in COMMANDS.items():
            for op,row in zip(make(),read(family)['results'],strict=True):
                total+=1;v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual([e['result']for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after'])
                if not op.get('cancel_before_history',True):
                    self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                after=v['after'];self.assertEqual(len(after),1 if op['delete_input']else 2)
                if not op['delete_input']:self.assertEqual(after[0],v['before'][0])
                out=after[-1];self.assertFalse(out['source']);self.assertEqual(out['selected'],0)
                self.assertEqual(len(out['surfaces']),1)
                s=out['surfaces'][0];self.assertEqual(s['degree'][0],3)
                self.assertTrue(all(p['weight']>0 for p in s['control_points']))
        self.assertEqual(total,172)

    def test_public_length_estimator_explains_profile_domain(self):
        estimates=read('revolve_domain_estimates')['results']
        for row in estimates:
            for value in row['value'].values():
                loose=next(r['length']for r in value['lengths']if r['tolerance']==1e-4)
                self.assertEqual(value['size'][2],loose)
        pilot=read('revolve_deformable_pilot')['results'][0]['value']
        self.assertIn('PointCount=10',pilot['history'])

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'),dict(point_count=True),dict(point_count=0),dict(delete_input=1),dict(extra=True),dict(curve=None)]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(cases()[0],**edit)]))
        op=copy.deepcopy(cases()[0]);op['curve']['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))

    def test_public_helpers_stage_and_import_without_native_runtime(self):
        for op in [cases()[0],dict(op='revolve_parameterization',id='line',case='line'),dict(op='revolve_domain_sampling',id='line',case='line')]:
            with tempfile.TemporaryDirectory()as directory:
                stage_helpers(dict(operations=[op]),HERE,Path(directory))
                module={'revolve_deformable':'revolve_deformable_probe','revolve_parameterization':'revolve_parameterization_probe','revolve_domain_sampling':'revolve_domain_sampling_probe'}[op['op']]
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',directory,module],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
