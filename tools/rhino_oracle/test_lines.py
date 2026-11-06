"""Frozen Lines inputs, private launch policy and strict qualification gates."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import lines_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_lines import HERE,ROOT,PROOF,FAMILIES,INCOMPLETE,INVALID,build
def observed(name):return json.loads(gzip.decompress((HERE/('observations/'+name+'.json.gz')).read_bytes()))

class LinesTests(unittest.TestCase):
    def test_prescribed_sources_and_frozen_producer_closures(self):
        self.assertEqual(json.loads((HERE/'fixtures/lines_sources.json').read_text()),cases.CASES)
        for family,count in FAMILIES.items():
            raw=(HERE/('fixtures/'+family+'.json')).read_bytes();data=observed(family)
            self.assertEqual(json.loads(raw),cases.request([r['id'] for r in data['results']]))
            self.assertEqual(len(data['results']),count);self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)

    def test_native_input_completion_history_invalid_objects_and_corrections(self):
        rows={r['id']:r['value'] for f in FAMILIES for r in observed(f)['results']};self.assertEqual(set(rows),set(cases.CASES));success=0;invalid=[]
        for k,v in rows.items():
            self.assertTrue(v['returned_idle']);self.assertEqual(v['created'],v['after']);self.assertEqual(v['after'],v['redo']);self.assertEqual(v['before'],v['undo'])
            if cases.recipe(k).get('macro'):
                self.assertFalse(v['input_trace']);success+=v['success'];continue
            t=v['input_trace'][0];actions=cases.recipe(k)['actions']
            if k in INCOMPLETE:
                corrected,n=INCOMPLETE[k];self.assertEqual(t['completed'],n);self.assertEqual(t['expected'],n+1);self.assertEqual(t['errors'],['incomplete owned Lines input']);self.assertEqual(v['after'],rows[corrected]['after'])
            else:
                self.assertFalse(t['errors']);self.assertEqual(t['completed'],t['expected']);self.assertEqual(len(t['sent']),len(actions));success+=v['success'] and k not in INVALID
            for action,step in zip(actions,t['sent']):
                if isinstance(action,list):self.assertEqual([float(n) for n in step['token'][1:].split(',')],action)
                elif isinstance(action,dict):self.assertEqual([float(n) for n in step['token'][1:].split(',')],action['relative'])
                else:self.assertEqual(step['token'],'_'+action)
            if any(not o['native_valid'] for o in v['after']):invalid.append(k)
        self.assertEqual(set(invalid),INVALID);self.assertEqual(success,33)
        self.assertEqual(len(rows['native_epsilon_below']['after']),2);self.assertEqual(len(rows['native_epsilon_at']['after']),3);self.assertEqual(len(rows['native_epsilon_above']['after']),3)
        for k in ['redo_cancel','redo_undo_all']:self.assertEqual(rows[k]['pending_redo'][-1]['point'],[90.,91.,92.])

    def test_invalid_unbounded_nonprivate_inputs_fail_before_launch(self):
        seed=cases.request()
        for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(extra='script')]:
            op=copy.deepcopy(seed['operations'][0]);op.update(changes)
            with self.assertRaises(ValueError):cases.validate(op)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
            for n in [True,1.0,2]:
                q=copy.deepcopy(seed);q['iterations']=n
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLinesInvalid').run_rhino(q)
            q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
            with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLinesInvalid').run_rhino(q)
            launch.assert_not_called()

    def test_rebuilt_qualification_keeps_geometry_validity_and_coverage_gates(self):
        path=ROOT/'docs/lines-validation.json'
        if not path.exists():self.skipTest('qualification follows the implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6);self.assertFalse(record['full_native_parity'])
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'lines-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines) if s.startswith('LINES_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate Lines replay'):build(proof,revision)
            def replace(**changes):
                row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy();changed[index]='LINES_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            for value in [1.1e-6,float('nan')]:
                replace(maximum_coefficient_and_sample_error=value)
                with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            replace(native_valid=False)
            with self.assertRaisesRegex(ValueError,'native-validity replay scope changed'):build(proof,revision)
            replace(archives=0)
            with self.assertRaisesRegex(ValueError,'archive coverage differs'):build(proof,revision)
if __name__=='__main__':unittest.main()
