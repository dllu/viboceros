"""Native command results, immutable provenance and isolated owned producers."""
import copy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .structural_sdk import stage_helpers, validate_operations
from .subd_direction_cases import cases, controls, source
from .subd_direction_replay import HERE, read, compare


class SubDDirectionTests(unittest.TestCase):
    def test_native_states_and_frozen_provenance(self):
        count = 0
        for suffix, recipes in [('discovery', cases()), ('controls', controls())]:
            family = 'subd_direction_' + suffix
            fixture = HERE / 'fixtures' / (family + '.json')
            request = json.loads(fixture.read_bytes())
            validate_operations(request)
            self.assertEqual(request['operations'], recipes)
            data = read(family)
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name, digest in data['provenance']['sources'].items():
                raw = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)
            for op, row in zip(recipes, data['results'], strict=True):
                count += 1
                self.assertEqual(op['id'], row['id'])
                v = row['value']
                compare(v['input'], source(op['scene']))
                self.assertNotIn('Unknown command', v['history'])
                events = [e for e in v['events'] if e['name'] == op['command']]
                self.assertEqual(len(events), 1)
                expected = 'Nothing' if op['command'] == 'Dir' and not op['mixed'] else 'Cancel' if op['cancel'] else 'Success'
                self.assertEqual(events[0]['result'], expected)
                self.assertEqual(v['success'], expected == 'Success')
                selected = op['preselect'] and not op['cancel']
                for phase in ('after', 'undo', 'redo'):
                    self.assertEqual([s['selected'] for s in v[phase]['sources']], [selected] * (1 + op['mixed']))
                before, after = v['before']['sources'][0]['geometry'], v['after']['sources'][0]['geometry']
                for key in ('points', 'vertex_tags', 'vertex_ids', 'edges', 'edge_tags', 'edge_ids', 'face_ids'):
                    self.assertEqual(before[key], after[key], (op['id'], key))
                if v['changed'] and op['scene'] not in ('cube', 'creased_cube'):
                    self.assertEqual(after['faces'], [f[:1] + list(reversed(f[1:])) for f in before['faces']])
                else:
                    self.assertEqual(before['faces'], after['faces'])
                self.assertEqual(v['redo']['points'], [] if v['changed'] else [[40., 50., 60.]])
        self.assertEqual(count, 82)

    def test_fixed_recipes_and_isolated_worker_import(self):
        for op in cases() + controls():
            for change in [dict(scene='external'), dict(command='Delete'), dict(preselect=1), dict(flat=0), dict(mixed=1), dict(cancel=0), dict(extra=True), dict(steps=['UReverse'])]:
                with self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op), **change)]))
        request = dict(protocol_version=1, iterations=1, operations=controls())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request, HERE, Path(folder))
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import subd_direction_probe', folder], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()
