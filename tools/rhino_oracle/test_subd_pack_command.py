"""Frozen owned face-pack commands, source provenance and settled history."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .subd_pack_command_probe import cases
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent
class SubDPackCommandTests(unittest.TestCase):
    def test_frozen_commands_sources_and_history(self):
        path=HERE/'fixtures/subd_pack_command.json';query=json.loads(path.read_bytes())
        data=json.loads(gzip.decompress((HERE/'observations/subd_pack_command.json.gz').read_bytes()))
        self.assertEqual(query['operations'],cases());validate_operations(query)
        self.assertEqual([r['id'] for r in data['results']],[r['id'] for r in cases()])
        self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        for name,digest in data['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
        count=0
        for op,row in zip(cases(),data['results']):
            self.assertEqual(len(row['value']['steps']),1 if op.get('cancel') else 2)
            for repeat,step in enumerate(row['value']['steps']):
                count+=1;cancel=bool(op.get('cancel'));post=op.get('postselect') or repeat>0
                self.assertEqual(step['success'],not cancel)
                self.assertEqual(step['results'],['Cancel' if cancel else 'Success'])
                originals={r['source']:r for r in step['before']}
                for obj in step['settled']:
                    for key in ('kind','name','layer','groups','color','crc'):
                        self.assertEqual(obj[key],originals[obj['source']][key])
                    self.assertEqual(obj['selected'],False if post else originals[obj['source']]['selected'])
                    if obj['kind']=='SubD':self.assertTrue(bytes(obj['archive']).startswith(b'3D Geometry File Format'))
                if cancel:
                    self.assertIsNone(step['undo']);self.assertIsNone(step['redo'])
                else:
                    self.assertEqual([o['source'] for o in step['undo']],[o['source'] for o in step['before']])
                    self.assertEqual([o['source'] for o in step['redo']],[o['source'] for o in step['settled']])
        self.assertEqual(count,33)

    def test_strict_recipes_and_standalone_import(self):
        query=json.loads((HERE/'fixtures/subd_pack_command.json').read_bytes())
        for changes in [dict(scene='external'),dict(extra=True),dict(postselect=1),dict(stale=1),dict(cancel=1)]:
            op=copy.deepcopy(cases()[-1] if 'cancel' in changes else cases()[12]);op.update(changes)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_pack_command_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
