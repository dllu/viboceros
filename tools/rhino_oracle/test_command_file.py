"""Fixed file inputs, private launch policy and strict qualification boundaries."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import command_file_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_command_file import HERE,ROOT,PROOF,FAMILIES,FAILURES,build

def observed(family):return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))

class CommandFileTests(unittest.TestCase):
    def test_fixed_sources_and_frozen_producers(self):
        self.assertEqual(json.loads((HERE/'fixtures/command_file_sources.json').read_text()),cases.CASES)
        for family,count in FAMILIES.items():
            raw=(HERE/('fixtures/'+family+'.json')).read_bytes();query=json.loads(raw);data=observed(family)
            self.assertEqual(query,cases.request([r['id'] for r in data['results']]))
            self.assertEqual(len(data['results']),count)
            self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                frozen=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)

    def test_complete_inputs_atomic_history_and_explicit_layer_redo_gap(self):
        successful=edited=0;gaps=[]
        for family in FAMILIES:
            for row in observed(family)['results']:
                k,v=row['id'],row['value'];recipe=cases.recipe(k)
                self.assertTrue(v['returned_idle']);self.assertEqual(v['success'],k not in FAILURES)
                traces=v['input_trace'];self.assertEqual(len(traces),int(bool(recipe.get('actions'))))
                for trace in traces:
                    self.assertFalse(trace['errors']);self.assertEqual(trace['completed'],trace['expected'])
                    self.assertEqual(len(trace['sent']),len(recipe['actions']))
                    for action,step in zip(recipe['actions'],trace['sent']):
                        if isinstance(action,list):self.assertEqual([float(n) for n in step['token'][1:].split(',')],action)
                        else:self.assertEqual(step['token'],'_'+action)
                if k not in {'bang_middle','unknown_command'}:self.assertNotIn('Unknown command',v['history'])
                changed=v['before']!=v['after'];successful+=v['success'];edited+=changed
                self.assertEqual(len(v['undo_steps']),int(changed));self.assertEqual(len(v['redo_steps']),int(changed))
                if changed:
                    self.assertEqual(v['undo_steps'][-1],v['before']);self.assertEqual(v['redo_steps'][-1],v['after'])
                if 'layers_after' in v and v['layers_after']!=v['layers_redo']:
                    gaps.append(k)
                    layer=next(l for l in v['layers_after'] if l['name']=='Macro layer')
                    self.assertFalse(layer['locked']);self.assertTrue(layer['current'])
                    layer=next(l for l in v['layers_redo'] if l['name']=='Macro layer')
                    self.assertTrue(layer['locked']);self.assertFalse(layer['current'])
        self.assertEqual((successful,edited),(24,26));self.assertEqual(gaps,['layer_options'])

    def test_invalid_nonprivate_and_unbounded_requests_fail_before_launch(self):
        seed=cases.request()
        for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(extra='script')]:
            op=copy.deepcopy(seed['operations'][0]);op.update(changes)
            with self.assertRaises(ValueError):cases.validate(op)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
            for iterations in [True,1.0,2]:
                q=copy.deepcopy(seed);q['iterations']=iterations
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleCommandInvalid').run_rhino(q)
            q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
            with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleCommandInvalid').run_rhino(q)
            launch.assert_not_called()

    def test_rebuilt_qualification_rejects_missing_geometry_and_metadata_evidence(self):
        path=ROOT/'docs/command-file-validation.json'
        if not path.exists():self.skipTest('qualification follows the implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision']
        self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6)
        self.assertEqual(record['layer_redo_gaps'],['layer_options']);self.assertFalse(record['full_native_parity'])
        with self.assertRaisesRegex(ValueError,'full implementation revision'):build(PROOF,'HEAD')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines) if s.startswith('COMMAND_FILE_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate UI'):build(proof,revision)
            def replace(**changes):
                row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy()
                changed[index]='COMMAND_FILE_UI '+json.dumps(row)+'\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            replace(maximum_coefficient_and_sample_error=1.1e-6)
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            replace(maximum_coefficient_and_sample_error=float('nan'))
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            replace(layer_history_matches_native=False)
            with self.assertRaisesRegex(ValueError,'layer Redo gap scope changed'):build(proof,revision)
            replace(undo_steps=0)
            with self.assertRaisesRegex(ValueError,'history or archive coverage differs'):build(proof,revision)

if __name__=='__main__':unittest.main()
