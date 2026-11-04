"""Frozen native loop commands, positive gestures, filter controls and provenance."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from .edge_loop_cases import cases, discovery_cases
from .edge_loop_gesture_cases import cases as gestures, filtered_cases, subobject_cases, modified_cases
from .edge_loop_gestures import EdgeLoopGesturePicker
from .structural_sdk import stage_helpers, validate_operations

HERE=Path(__file__).resolve().parent
FAMILIES=[('edge_loop_discovery',discovery_cases()),('edge_loop_commands',cases()),
          ('edge_loop_gestures',gestures()),('edge_loop_filtered_gestures',filtered_cases()),
          ('edge_loop_subobject_gestures',subobject_cases()),('edge_loop_modified_gestures',modified_cases())]

class EdgeLoopsTests(unittest.TestCase):
    def test_fixed_native_requests_sources_complete_gestures_and_output_order(self):
        count=0
        for family,recipes in FAMILIES:
            fixture=HERE/'fixtures'/(family+'.json');query=json.loads(fixture.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],recipes);validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                count+=1;v=row['value'];self.assertIs(v['source_unchanged'],True)
                self.assertFalse(v['after']['whole'])
                if 'to_nurbs_order' in v:self.assertEqual(v['to_nurbs_order'],list(reversed(v['after']['ordinals'])))
                if 'gesture' in family:
                    self.assertEqual(len(v['trace']),2)
                    if family in ('edge_loop_gestures','edge_loop_subobject_gestures'):
                        self.assertEqual(v['after'],v['before'])
                    if family=='edge_loop_filtered_gestures':self.assertGreater(len(v['after']['ordinals']),len(v['before']['ordinals']) if row['id'].endswith('add') else 0)
        self.assertEqual(count,92)

    def test_strict_recipes_and_standalone_worker_import(self):
        for family,recipes in FAMILIES:
            query=json.loads((HERE/'fixtures'/(family+'.json')).read_bytes())
            for change in [dict(pair=[float(i) for i in recipes[0]['pair']]),dict(kind='external'),dict(extra=True),dict(scene='external')]:
                op=copy.deepcopy(recipes[0]);op.update(change)
                with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(query,HERE,Path(folder))
                subprocess.run([sys.executable,'-I','-c',
                    'import sys;sys.path.insert(0,sys.argv[1]);import edge_loop_probe,edge_loop_cases',folder],check=True,capture_output=True)

    def test_owned_double_click_keys_are_released_on_success_and_failure(self):
        for filtered,mode,expected in [(False,None,['ctrl','shift']),(True,None,[]),(False,'shift',['shift']),(False,'ctrl',['ctrl'])]:
            for fail in [False,True]:
                calls=[]
                def run(command,**kwargs):
                    calls.append(command)
                    if fail and command[1]=='click':raise subprocess.CalledProcessError(1,command)
                picker=EdgeLoopGesturePicker(filtered,{'owned':mode} if mode else None)
                with patch('tools.rhino_oracle.edge_loop_gestures.subprocess.run',side_effect=run),patch('tools.rhino_oracle.edge_loop_gestures.time.sleep'):
                    if fail:
                        with self.assertRaises(subprocess.CalledProcessError):picker.send_input('@edge-loop-double:owned',10,20,'123')
                    else:self.assertTrue(picker.send_input('@edge-loop-double:owned',10,20,'123'))
                self.assertEqual(calls[0],['xdotool','windowactivate','--sync','123'])
                self.assertEqual([c[-1] for c in calls if c[1]=='keydown'],expected)
                self.assertEqual([c[-1] for c in calls if c[1]=='keyup'],list(reversed(expected)))
                self.assertIn(['xdotool','mouseup','1'],calls)
        with patch('tools.rhino_oracle.edge_loop_gestures.subprocess.run') as run:
            with self.assertRaises(ValueError):EdgeLoopGesturePicker().send_input('@edge-loop-double:bad:extra',10,20,'123')
            run.assert_not_called()

if __name__=='__main__':unittest.main()
