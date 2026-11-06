"""Frozen grid inputs, private launch policy and strict qualification gates."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import point_grid_input_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_point_grid_input import HERE,ROOT,PROOF,FAMILIES,DIAGNOSTIC,build

def observed(family):return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))

class PointGridInputTests(unittest.TestCase):
    def test_fixed_recipes_and_frozen_producer_closure(self):
        for family,count in FAMILIES.items():
            raw=(HERE/('fixtures/'+family+'.json')).read_bytes();query=json.loads(raw);data=observed(family)
            names=[o['id'] for o in query['operations']]
            self.assertEqual(query,cases.request(names));self.assertEqual(len(data['results']),count)
            self.assertEqual(names,[r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                frozen=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)

    def test_complete_independent_inputs_and_native_document_policy(self):
        success=controls=0
        for family in FAMILIES:
            for row in observed(family)['results']:
                k,v=row['id'],row['value'];trace=v['input_trace'][0]
                if k==DIAGNOSTIC:
                    self.assertEqual(trace['completed'],1);self.assertEqual(len(trace['errors']),2)
                    self.assertFalse(v['success']);self.assertEqual(v['after'],v['before'])
                    self.assertIn('Row degree',v['history']);continue
                self.assertTrue(v['returned_idle']);self.assertFalse(trace['errors'])
                self.assertEqual(trace['completed'],trace['expected']);recipe=cases.recipe(k)
                self.assertEqual(len(trace['sent']),len(recipe['actions']))
                for action,step in zip(recipe['actions'],trace['sent']):
                    token=step['token']
                    if isinstance(action,list):self.assertEqual([float(n) for n in token[1:].split(',')],action)
                    elif isinstance(action,str):self.assertEqual(token,'_'+action)
                    else:self.assertEqual(float(token),action)
                self.assertNotIn('Unknown command',v['history']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(v['created'],v['after'])
                if v['success']:
                    success+=1;self.assertEqual(v['before'],v['undo']);self.assertEqual(v['before'],v['after'][:1])
                    for obj in v['after'][1:]:self.assertTrue(obj['native_valid']);self.assertEqual(obj['selected'],0)
                else:controls+=1;self.assertEqual(v['after'],v['before'])
        self.assertEqual((success,controls),(18,6))

    def test_private_bounded_fixed_inputs_fail_before_launch(self):
        seed=cases.request()
        for changes in [dict(case=None),dict(case='foreign'),dict(extra='script')]:
            op=copy.deepcopy(seed['operations'][0]);op.update(changes)
            with self.assertRaises(ValueError):cases.validate(op)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
            for iterations in [True,1.0,2]:
                q=copy.deepcopy(seed);q['iterations']=iterations
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleGridInvalid').run_rhino(q)
            launch.assert_not_called()

    def test_qualification_excludes_timeout_without_weakening_geometry_gate(self):
        path=ROOT/'docs/point-grid-input-validation.json'
        if not path.exists():self.skipTest('qualification follows the implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision']
        self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6)
        self.assertEqual(record['diagnostics'][0]['id'],DIAGNOSTIC);self.assertFalse(record['diagnostics'][0]['qualified'])
        self.assertFalse(record['full_native_parity'])
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines) if s.startswith('POINT_GRID_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate UI'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=1.1e-6
            changed=lines.copy();changed[index]='POINT_GRID_UI '+json.dumps(row)+'\n'
            path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)

if __name__=='__main__':unittest.main()
