"""Fixed Line inputs, capture integrity, isolated execution and qualification gates."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from unittest import mock
from pathlib import Path
from . import line_construction_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_line_construction import HERE,ROOT,PROOF,FAMILIES,INCOMPLETE,INVALID,ALL_INVALID,EARLY,build,observations

class LineConstructionTests(unittest.TestCase):
    def test_fixed_inputs_and_frozen_capture_sources(self):
        self.assertEqual(json.loads((HERE/'fixtures/line_construction_sources.json').read_bytes()),cases.CASES)
        for family,count in FAMILIES.items():
            fixture=(HERE/('fixtures/'+family+'.json')).read_bytes();data=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
            self.assertEqual(len(data['results']),count);self.assertEqual(json.loads(fixture),cases.request([r['id'] for r in data['results']]))
            self.assertEqual(hashlib.sha256(fixture).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)

    def test_native_completion_cancellation_validity_domains_and_corrections(self):
        rows=observations();self.assertEqual(set(rows),set(cases.CASES));self.assertEqual(len(rows),68)
        complete={k:v for k,v in rows.items() if k not in INCOMPLETE};self.assertEqual(len(complete),54)
        self.assertEqual(sum(v['success'] for v in complete.values()),43)
        for k,v in rows.items():
            self.assertTrue(v['returned_idle']);self.assertEqual(v['created'],v['after']);self.assertEqual(v['after'],v['redo']);self.assertEqual(v['before'],v['undo'])
            self.assertEqual(all(o['native_valid'] for o in v['after']),k not in ALL_INVALID)
            if cases.recipe(k).get('macro'):self.assertFalse(v['input_trace']);continue
            t=v['input_trace'][0];actions=cases.recipe(k)['actions']
            if k in INCOMPLETE:
                n,expected,errors,corrected=INCOMPLETE[k];self.assertEqual((t['completed'],t['expected'],t['errors']),(n,expected,errors))
                if errors==EARLY:self.assertEqual(v['after'],rows[corrected]['after']);self.assertEqual(v['success'],rows[corrected]['success'])
            else:self.assertEqual(t['completed'],t['expected']);self.assertFalse(t['errors']);self.assertEqual(len(t['sent']),len(actions))
            for action,step in zip(actions,t['sent']):
                if isinstance(action,list):self.assertEqual([float(n) for n in step['token'][1:].split(',')],action)
                elif isinstance(action,str):self.assertEqual(step['token'],'_'+action)
                else:self.assertEqual(float(step['token']),action)
        for k in INVALID:self.assertEqual(rows[k]['after'][-1]['curve']['domain'],[0.,1.])
        self.assertEqual(rows['tiny_complete_1e9']['after'][-1]['curve']['domain'],[0.,1e-9])
        self.assertTrue(rows['macro_vertical']['success']);self.assertEqual(len(rows['macro_vertical']['after']),3)
        self.assertFalse(rows['macro_cancel']['success']);self.assertEqual(rows['macro_cancel']['after'],rows['macro_cancel']['before'])

    def test_unbounded_foreign_and_nonprivate_requests_fail_before_launch(self):
        seed=cases.request()
        for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(extra='script')]:
            op=copy.deepcopy(seed['operations'][0]);op.update(changes)
            with self.assertRaises(ValueError):cases.validate(op)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
            for n in [True,1.0,2]:
                q=copy.deepcopy(seed);q['iterations']=n
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineInvalid').run_rhino(q)
            q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
            with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineInvalid').run_rhino(q)
            launch.assert_not_called()

    def test_rebuilt_qualification_enforces_geometry_coverage_validity_and_archives(self):
        path=ROOT/'docs/line-construction-validation.json'
        if not path.exists():self.skipTest('qualification follows the implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6);self.assertFalse(record['full_native_parity'])
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'line-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines) if s.startswith('LINE_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate Line replay'):build(proof,revision)
            def replace(**changes):
                row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy();changed[index]='LINE_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            for value in [1.1e-6,float('nan')]:
                replace(maximum_coefficient_and_sample_error=value)
                with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            replace(native_valid=False)
            with self.assertRaisesRegex(ValueError,'native-validity replay scope changed'):build(proof,revision)
            replace(archives=0)
            with self.assertRaisesRegex(ValueError,'archive coverage differs'):build(proof,revision)
if __name__=='__main__':unittest.main()
