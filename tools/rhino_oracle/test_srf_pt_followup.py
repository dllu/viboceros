"""Remote triangle admission and public SDK orientation evidence."""
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from . import srf_pt_cases as cases
from .report_srf_pt_followup import HERE,ROOT,PROOF,FACE_GAPS,RELATIVE_REJECTS,build

def observed():
    return json.loads(gzip.decompress((HERE/'observations/srf_pt_followup.json.gz').read_bytes()))

class SrfPtFollowupTests(unittest.TestCase):
    def test_literal_recipes_and_frozen_producers(self):
        raw=(HERE/'fixtures/srf_pt_followup.json').read_bytes();query=json.loads(raw);data=observed()
        self.assertEqual(query,cases.request(list(cases.FOLLOWUP)))
        self.assertEqual([o['id'] for o in query['operations']],[r['id'] for r in data['results']])
        self.assertEqual(len(data['results']),37)
        self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
        for digest in data['provenance']['sources'].values():
            frozen=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)

    def test_prescribed_inputs_and_atomic_native_document(self):
        successes=0
        for row in observed()['results']:
            v=row['value'];trace=v['input_trace'][0];recipe=cases.recipe(row['id'])
            self.assertTrue(v['returned_idle']);self.assertFalse(trace['errors'])
            self.assertEqual(trace['completed'],trace['expected']);self.assertEqual(len(trace['sent']),len(recipe['actions']))
            for action,step in zip(recipe['actions'],trace['sent']):
                if isinstance(action,str):self.assertEqual(step['token'],'_'+action)
                else:self.assertEqual([float(n) for n in step['token'][1:].split(',')],action)
            self.assertNotIn('Unknown command',v['history']);self.assertEqual(v['created'],v['after'])
            self.assertEqual(v['after'],v['redo']);self.assertEqual(v['model_tolerance'],cases.request()['tolerance'])
            if v['success']:
                successes+=1;self.assertEqual(v['before'],v['undo']);self.assertEqual(v['before'],v['after'][:-1])
                self.assertTrue(v['after'][-1]['native_valid'])
            else:self.assertEqual(v['after'],v['before'])
        self.assertEqual(successes,31)

    def test_relative_coincidence_has_rejected_and_accepted_neighbors(self):
        actual={r['id']:r['value']['success'] for r in observed()['results'] if r['id'].startswith('relative_triangle_')}
        self.assertEqual({k for k,v in actual.items() if not v},RELATIVE_REJECTS)
        self.assertEqual({k for k,v in actual.items() if v},{'relative_triangle_1e9_open','relative_triangle_1e12_above','relative_triangle_1e12_open'})

    def test_sdk_records_separate_surface_construction_from_document_orientation(self):
        flipped=unflipped=0
        for row in observed()['results']:
            v=row['value']
            if not v['success']:continue
            sdk=v['after'][-1]['sdk']
            if not sdk['base_solid']:continue
            self.assertTrue(sdk['base_valid']);self.assertFalse(sdk['base_reversed'])
            self.assertEqual(sdk['command_orientation'],'Outward')
            self.assertEqual(sdk['command_reversed'],sdk['base_orientation']=='Inward')
            self.assertEqual(row['id'] in FACE_GAPS,not sdk['command_reversed'])
            if sdk['command_reversed']:flipped+=1
            else:unflipped+=1
        self.assertEqual((flipped,unflipped),(21,7))

    def test_qualification_preserves_geometry_gate_and_named_face_gaps(self):
        path=ROOT/'docs/srf-pt-followup-validation.json'
        if not path.exists():self.skipTest('qualification follows the validated implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision']
        self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['native_observations'],97)
        self.assertEqual(record['model_epsilon'],1e-6);self.assertFalse(record['full_native_parity'])
        self.assertEqual(set(record['native_face_sense_gaps']),FACE_GAPS)
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines) if s.startswith('SRF_PT_FOLLOWUP_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete command or UI replay'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=1.1e-6
            changed=lines.copy();changed[index]='SRF_PT_FOLLOWUP_UI '+json.dumps(row)+'\n'
            path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['face_sense_matches_native']=False
            changed=lines.copy();changed[index]='SRF_PT_FOLLOWUP_UI '+json.dumps(row)+'\n'
            path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'face-sense scope changed'):build(proof,revision)

if __name__=='__main__':unittest.main()
