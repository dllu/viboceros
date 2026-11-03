"""Frozen actual-command geometry, lifecycle, defaults and owned dispatch."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .subd_nurbs_command_cases import cases,lifecycle_cases
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent
def read(name):return json.loads(gzip.decompress((HERE/'observations'/(name+'.json.gz')).read_bytes()))
def metadata(snapshot):
    value=copy.deepcopy(snapshot)
    for row in value['objects']:
        row.pop('brep',None);row.pop('subd',None)
    return value

class SubDNurbsCommandTests(unittest.TestCase):
    def test_frozen_requests_sources_commands_and_history(self):
        count=0
        for family,recipes in [('subd_nurbs_command',cases()),('subd_nurbs_lifecycle',lifecycle_cases())]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes());data=read(family)
            self.assertEqual(query['operations'],recipes);validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']],[r['id'] for r in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                for step in row['value']['steps']:
                    count+=1;cancel=bool(step['options'].get('cancel'));self.assertEqual(step['success'],not cancel)
                    self.assertEqual(step['results'],['Cancel' if cancel else 'Success'])
                    for obj in step['before']['objects']:
                        if 'subd' in obj:self.assertIs(obj['subd']['valid'],True)
                    sources={o['source']:o for o in step['before']['objects']}
                    for obj in step['after']['objects']:
                        if obj['kind']=='SubD':
                            self.assertEqual(obj['crc'],sources[obj['source']]['crc'])
                            self.assertEqual(obj['subd'],sources[obj['source']]['subd'])
                    if cancel:
                        if 'after_script' in step:self.assertEqual(step['after_script'],metadata(step['before']))
                    else:
                        self.assertEqual(step['undo'],metadata(step['before']))
                        if 'after_script' in step:self.assertEqual(step['redo'],step['after_script'])
        self.assertEqual(count,61)

    def test_bootstrap_geometry_and_partial_option_memory(self):
        sdk={r['id']:r['value']['output']['geometry'] for r in read('subd_packing')['results']}
        for family,key,reference in [('subd_nurbs_command','bootstrap','subdivided_box_3'),
                ('subd_nurbs_lifecycle','lifecycle_bootstrap','fan6_3')]:
            rows={r['id']:r['value'] for r in read(family)['results']}
            output=next(o['brep'] for o in rows[key]['steps'][0]['after']['objects'] if 'brep' in o)
            self.assertEqual(output,sdk[reference])
        rows={r['id']:r['value'] for r in read('subd_nurbs_command')['results']}
        steps=rows['memory']['steps']
        for a,b in [(0,1),(2,3),(4,5),(6,7)]:
            output=lambda i:next(o['brep'] for o in steps[i]['after']['objects'] if 'brep' in o)
            self.assertEqual(output(a),output(b))
        self.assertEqual([len(next(o['brep'] for o in s['after']['objects'] if 'brep' in o)['faces']) for s in steps],[24,24,6,6,6,6,6,6])

    def test_strict_recipes_and_standalone_worker_import(self):
        query=json.loads((HERE/'fixtures/subd_nurbs_command.json').read_bytes())
        for changes in [dict(scene='external'),dict(extra=True),dict(steps=[dict(packed=1,mode='G1x',delete=False)])]:
            op=copy.deepcopy(query['operations'][2]);op.update(changes)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_nurbs_command_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
