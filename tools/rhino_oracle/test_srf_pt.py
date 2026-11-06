"""Frozen corner-surface recipes, document policy and qualification boundaries."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock
from . import srf_pt_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_srf_pt import FAMILIES,HERE,ROOT,PROOF,build

def observed(name):
    return json.loads(gzip.decompress((HERE/('observations/'+name+'.json.gz')).read_bytes()))

class SrfPtTests(unittest.TestCase):
    def test_fixed_recipes_and_every_frozen_producer(self):
        total=0
        for family,count in FAMILIES.items():
            raw=(HERE/('fixtures/'+family+'.json')).read_bytes();query=json.loads(raw);data=observed(family)
            names=[o['id'] for o in query['operations']];self.assertEqual(query,cases.request(names))
            self.assertEqual(len(data['results']),count);total+=count
            self.assertEqual(names,[r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                frozen=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)
        self.assertEqual(total,60)

    def test_inputs_are_prescribed_complete_and_independent_of_geometry(self):
        constructed=controls=0
        for family in FAMILIES:
            for row in observed(family)['results']:
                case=cases.recipe(row['id']);v=row['value'];trace=v['input_trace'][0]
                self.assertTrue(v['returned_idle']);self.assertFalse(trace['errors'])
                self.assertEqual(trace['completed'],trace['expected'])
                sent=trace['sent'];self.assertEqual(len(sent),len(case['actions']))
                for action,step in zip(case['actions'],sent):
                    token=step['token']
                    if isinstance(action,str):self.assertEqual(token,'_'+action)
                    elif isinstance(action,list):self.assertEqual([float(n) for n in token[1:].split(',')],action)
                    else:self.assertEqual(float(token),action)
                self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['model_tolerance'],cases.request()['tolerance'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['after'],v['redo'])
                if v['success']:
                    constructed+=1;self.assertEqual(v['before'],v['undo'])
                    self.assertEqual(v['before'],v['after'][:-1]);o=v['after'][-1]
                    self.assertTrue(o['native_valid']);self.assertEqual(o['face_count'],1)
                    self.assertEqual(o['surfaces'][0]['degree'],[1,1]);self.assertEqual(o['surfaces'][0]['control_count'],[2,2])
                    self.assertEqual(o['attributes'],dict(name='',layer='Plane output',color_source='ColorFromLayer',color=[0,0,0],groups=[]))
                else:controls+=1;self.assertEqual(v['after'],v['before'])
        self.assertEqual((constructed,controls),(40,20))

    def test_invalid_or_nonprivate_requests_fail_before_launch(self):
        seed=cases.request()
        for change in [dict(case=[]),dict(case=None),dict(id='foreign'),dict(extra='script')]:
            op=copy.deepcopy(seed['operations'][0]);op.update(change)
            with self.assertRaises(ValueError):cases.validate(op)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
            for iterations in [True,1.0,2]:
                q=copy.deepcopy(seed);q['iterations']=iterations
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSrfPtInvalid').run_rhino(q)
            q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
            with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSrfPtInvalid').run_rhino(q)
            launch.assert_not_called()

    def test_qualification_keeps_geometry_gate_and_exposes_the_face_sense_gap(self):
        path=ROOT/'docs/srf-pt-validation.json'
        if not path.exists():self.skipTest('qualification follows the validated implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision']
        self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6)
        self.assertEqual(record['native_face_sense_gaps'],['tiny_translated']);self.assertFalse(record['full_native_parity'])
        with self.assertRaisesRegex(ValueError,'full implementation revision'):build(PROOF,'HEAD')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines) if s.startswith('SRF_PT_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete command or UI replay'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=1.1e-6
            changed=lines.copy();changed[index]='SRF_PT_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['face_sense_matches_native']=False
            changed=lines.copy();changed[index]='SRF_PT_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'face-sense scope changed'):build(proof,revision)

if __name__=='__main__':unittest.main()
