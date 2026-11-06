"""Completion recipes, independent pointer context and frozen evidence gates."""
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from .plane_completion_cases import COMPLETION_CASES
from .plane_surface_cases import request
from .report_plane_completion import FAMILIES, HERE, PROOF, ROOT, build

def observed(family):
    path=HERE/('observations/'+family+'.json')
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))

class PlaneCompletionTests(unittest.TestCase):
    def test_capture_recipes_and_frozen_producer_closures(self):
        for family,count in FAMILIES.items():
            data=observed(family);raw=(HERE/('fixtures/'+family+'.json')).read_bytes();query=json.loads(raw)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in query['operations']])
            self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                frozen=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest)
            if family=='plane_completion_terminal':
                self.assertEqual(query,request(list(COMPLETION_CASES)))
                from .structural_sdk import PROBES
                module,deps=PROBES['plane_surface_command']
                self.assertTrue({n+'.py' for n in [module,*deps]}<=data['provenance']['sources'].keys())

    def test_terminal_inputs_consume_prescribed_tokens_with_stable_independent_camera_rays(self):
        data=observed('plane_completion_terminal')
        successes=0
        for row in data['results']:
            v=row['value'];case=COMPLETION_CASES[row['id']];trace=v['input_trace'][0]
            self.assertEqual(v['success'],case['expected_success'],row['id'])
            self.assertTrue(v['returned_idle']);self.assertFalse(trace['errors'])
            self.assertEqual(trace['completed'],trace['expected'])
            sent=[s['token'] for s in trace['sent']];expected=[]
            for a in case['actions']:
                expected.append(a)
                if a=='AroundCurve' and not case.get('skip_guide_pick'): expected.append('@guide')
            self.assertEqual(len(sent),len(expected))
            for token,a in zip(sent,expected):
                if a=='@guide': self.assertTrue(token.startswith("'_-SelID "))
                elif isinstance(a,str): self.assertEqual(token,'_'+a)
                elif isinstance(a,list): self.assertEqual([float(x) for x in token[1:].split(',')],a)
                else: self.assertEqual(float(token),a)
            self.assertEqual(v['pointer']['pixel'],v['pointer_after']['pixel'])
            pointer=v['pointer'];ray=pointer['ray'];camera=pointer['camera']
            if ray['forward_only']:
                self.assertEqual(ray['origin'],camera['location'])
                self.assertGreater(sum(a*b for a,b in zip(ray['direction'],camera['direction'])),0)
            self.assertTrue(all(o['native_valid'] for o in v['after']))
            if v['success']:
                successes+=1;self.assertEqual(v['after'],v['redo']);self.assertEqual(len(v['after']),len(v['before'])+1)
        self.assertEqual(successes,42)

    def test_pinned_qualification_requires_every_execution_and_keeps_geometry_gate(self):
        path=ROOT/'docs/plane-completion-validation.json'
        if not path.exists(): self.skipTest('qualification follows the validated implementation commit')
        record=json.loads(path.read_text());revision=record['implementation_revision']
        self.assertEqual(record,build(PROOF,revision))
        self.assertEqual((record['qualified_workflows'],record['script_constructions'],record['archive_round_trips_per_build']),(53,39,42))
        with self.assertRaisesRegex(ValueError,'full implementation revision'):build(PROOF,'HEAD')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            log=proof/'ui-portable.log.gz';lines=gzip.decompress(log.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines) if s.startswith('PLANE_COMPLETION_UI '))
            log.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete CLI or UI execution'):build(proof,revision)
            row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=1.1e-6
            lines[index]='PLANE_COMPLETION_UI '+json.dumps(row)+'\n'
            log.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)

if __name__=='__main__':unittest.main()
