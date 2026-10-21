"""Immutable native SubD append inputs and complete current qualification."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest import mock
from . import face_create_probe as probe
from .client import OracleClient,OracleProtocolError
from .report_subd_append import GROUPS,NORMAL_GROUP,ROOT,summarize
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class SubDAppendTests(unittest.TestCase):
    def test_fixed_inputs_and_archived_native_producers(self):
        for (name,count),factory in zip(GROUPS+[NORMAL_GROUP],[probe.subd_append_request,probe.subd_append_extra_request,probe.subd_draft_request,probe.subd_normal_request]):
            path=HERE/'fixtures'/(name+'.json');request=json.loads(path.read_text());self.assertEqual(request,factory());validate_operations(request)
            response=json.loads((HERE/'observations'/(name+'.json')).read_text());self.assertEqual(len(response['results']),count)
            self.assertEqual(response['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([row['id']for row in response['results']],[op['id']for op in request['operations']])
            for source,sha in response['provenance']['sources'].items():
                data=(HERE/source).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,source)
            for row in response['results']:
                value=row['value'];self.assertEqual(value['input'],probe.SCENES[row['id']]);self.assertTrue(value['success'])
                self.assertEqual(value['after_script'],value['redo']);self.assertNotIn('Unknown command:',value['history'])
                self.assertEqual(len([event for event in value['events']if event['name']=='3DFace']),1)
                if name.endswith('drafts'):
                    self.assertGreater(value['input_trace']['requests'],0)
                    self.assertTrue(all(prompt.startswith('Pick point')for prompt in value['input_trace']['prompts']))
                for output in value['after_script']:
                    if 'subd'in output:self.assertTrue(output['subd']['valid'])

    def test_physical_inputs_require_the_exact_owned_sequence(self):
        with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
            client=OracleClient(settings_scheme='VibocerosOracleSubDAppendTest20261021')
            for kind in ['reverse','mix','repeat','duplicate','subset']:
                request=probe.subd_draft_request()
                if kind=='reverse':request['operations'].reverse()
                elif kind=='mix':request['operations'].append(probe.subd_append_request()['operations'][0])
                elif kind=='repeat':request['iterations']=2
                elif kind=='duplicate':request['operations'].append(copy.deepcopy(request['operations'][0]))
                else:request['operations']=request['operations'][:-1]
                with self.assertRaises(OracleProtocolError):client.run_rhino(request)
            spawn.assert_not_called()

    def lines(self):
        report=json.loads((ROOT/'docs/subd-append-qualification.json').read_text())
        return report,(['SUBD_APPEND_'+stage.upper()+' '+json.dumps(row)for stage,rows in report['stages'].items()for row in rows]+['SUBD_NORMAL_COMMAND '+json.dumps(row)for row in report['normal_stages']['command']])

    def test_complete_current_qualification(self):
        report,lines=self.lines();self.assertEqual(summarize('\n'.join(lines)),report)
        self.assertEqual(report['recipes'],32);self.assertEqual(report['model_epsilon'],1e-6)
        self.assertEqual(report['normal_cases'],7);self.assertEqual(report['normal_epsilon'],1e-6)
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for path,sha in report['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)

    def test_missing_failed_or_hidden_outputs_cannot_qualify(self):
        report,lines=self.lines()
        for invalid in [lines[:-1],lines+lines[-1:],lines[::-1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        for key,bad in [('qualified',False),('maximum_sampled_surface_error',float('nan')),('maximum_control_or_limit_point_error',True),('maximum_sampled_surface_error',1.1e-6),('faces',0),('surface_sampled',False)]:
            row=copy.deepcopy(report['stages']['command'][0]);row['check']['objects'][0][key]=bad
            invalid=lines[:];invalid[0]='SUBD_APPEND_COMMAND '+json.dumps(row)
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        for key,bad in [('qualified',False),('maximum_corner_normal_error',float('nan')),('maximum_corner_normal_error',True),('maximum_corner_normal_error',1.1e-6),('corners',0)]:
            row=copy.deepcopy(report['normal_stages']['command'][0]);row['normals'][key]=bad
            invalid=lines[:];index=next(i for i,line in enumerate(invalid)if line.startswith('SUBD_NORMAL_COMMAND '));invalid[index]='SUBD_NORMAL_COMMAND '+json.dumps(row)
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        row=copy.deepcopy(report['stages']['command'][6]);row['check']['objects'].pop();invalid=lines[:];invalid[6]='SUBD_APPEND_COMMAND '+json.dumps(row)
        with self.assertRaises(ValueError):summarize('\n'.join(invalid))


if __name__=='__main__':unittest.main()
