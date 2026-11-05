"""Immutable native preference/creator cases and isolated public-SDK staging."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .use_extrusions_cases import cases
from .use_extrusions_creators_cases import cases as creator_cases,limits_cases,domain_cases
from .structural_sdk import stage_helpers,validate_operations
from .subd_edge_analysis_replay import HERE,read


class UseExtrusionsTests(unittest.TestCase):
    def test_native_requests_and_frozen_producers(self):
        for family,recipes in [('use_extrusions',cases()),('use_extrusions_creators_pilot',creator_cases(True)),('use_extrusions_creators',creator_cases()),('use_extrusions_creators_limits',limits_cases()),('use_extrusions_creators_domains',domain_cases())]:
            data=read(family);path=HERE/('fixtures/'+family+'.json');query=json.loads(path.read_bytes())
            self.assertEqual(query['operations'],recipes);validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']],[op['id']for op in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)

    def test_native_preference_keeps_selection_and_pending_redo(self):
        for op,row in zip(cases(),read('use_extrusions')['results'],strict=True):
            v=row['value'];expected=op['answer']=='Extrusion' if op['answer'] in ('Extrusion','Polysurface') else op['initial']=='Extrusion'
            self.assertIs(v['after']['use_extrusions'],expected);self.assertEqual(v['after']['points'],v['before']['points'])
            self.assertIs(v['success'],op['answer']!='Cancel');self.assertIs(v['redone'],True)
            self.assertEqual(v['redo']['points'],v['after']['points']+[dict(point=[10.,20.,30.],selected=0)])
            self.assertEqual([e['result']for e in v['events']],['Cancel'if op['answer']=='Cancel'else'Success'])

    def test_native_creator_lifecycle_and_mode_eligibility(self):
        counts=dict(extrusion=0,brep=0)
        for op,row in zip(creator_cases(),read('use_extrusions_creators')['results'],strict=True):
            v=row['value'];self.assertIs(v['success'],True);self.assertNotIn('Unknown command:',v['history'])
            self.assertEqual(v['undo'],v['before'],row['id']);self.assertEqual(v['redo'],v['after'],row['id'])
            for output in [o for o in v['after'] if not o['source']]:
                counts[output['kind']]+=1
                if op['mode']=='Polysurface' or op['recipe']=='surface' or op['recipe'].endswith('Round_0') or op['recipe'] in ('creator_detail_extrude_nonplanar','creator_detail_oblique_rectangle','creator_detail_oblique_circle'):
                    self.assertEqual(output['kind'],'brep',row['id'])
                else:self.assertEqual(output['kind'],'extrusion',row['id'])
        self.assertEqual(counts,dict(extrusion=77,brep=95))

    def test_native_pipe_stations_taper_and_custom_domains(self):
        for row in read('use_extrusions_creators_limits')['results']:
            self.assertEqual([o['kind']for o in row['value']['after']if not o['source']],['brep'])
        for row in read('use_extrusions_creators_domains')['results']:
            v=row['value'];output=next(o for o in v['after']if not o['source'])
            if row['id'].startswith('Extrusion_'):
                knots=output['extrusion']['surface']['knots_v']
                self.assertEqual([knots[0],knots[-1]],v['input']['rail_domain'])
            self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['after'])

    def test_arbitrary_input_rejected_and_helpers_staged_independently(self):
        for recipe in cases()+creator_cases():
            for edit in [dict(extra=True),dict(id='unowned'),dict(mode=[]),dict(recipe='../outside')]:
                with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(recipe),**edit)]))
        request=dict(protocol_version=1,iterations=1,operations=cases()+creator_cases())
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import use_extrusions_probe,use_extrusions_creators_probe',directory],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
