"""Retain soft-crease discovery inputs, native lifecycle and producer bytes."""
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from . import soft_crease_probe as probe
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent
FAMILY = 'soft_crease_discovery'


class SoftCreaseDiscoveryTests(unittest.TestCase):
    def native(self):
        return json.loads((HERE / 'observations' / (FAMILY + '.json')).read_text())

    def test_saved_inputs_and_native_archives_are_exact(self):
        path = HERE / 'fixtures' / (FAMILY + '.json')
        query = json.loads(path.read_text())
        self.assertEqual(query, probe.request())
        validate_operations(query)
        native = self.native()
        self.assertEqual(native['engine'], 'rhino')
        self.assertEqual(native['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(native['results']), 32)
        self.assertEqual([r['id'] for r in native['results']],
                         [op['id'] for op in query['operations']])
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),
                         native['provenance']['request_sha256'])
        for row in native['results']:
            self.assertEqual(row['value']['input'], probe.all_cases()[row['id']])
            for key, folder in [('source_3dm', 'source'), ('result_3dm', 'result')]:
                data = (HERE / 'fixtures' / FAMILY / folder / (row['id'] + '.3dm')).read_bytes()
                self.assertEqual(data, base64.b64decode(row['value'][key], validate=True))

    def test_original_producers_remain_available_after_refactoring(self):
        sources = self.native()['provenance']['sources']
        self.assertTrue({'subd_edit_scene.py', 'soft_crease_cases.py',
                         'soft_crease_probe.py', 'rhino_worker.py'} <= set(sources))
        for name, digest in sources.items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest:
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_cancel_and_noop_do_not_consume_a_model_history_record(self):
        no_change = {'soft_constant_0', 'soft_default', 'soft_cancel_empty',
                     'soft_cancel_after_weight', 'soft_boundary',
                     'soft_whole_object', 'soft_vertex_input'}
        cancelled = no_change - {'soft_constant_0', 'soft_default'}
        for row in self.native()['results']:
            key, value = row['id'], row['value']
            events = [e for e in value['events'] if e['name'] == 'SubDCrease']
            self.assertEqual(len(events), 1, key)
            self.assertEqual(events[0]['objects'], value['after'])
            self.assertIs(value['success'], key not in cancelled)
            self.assertEqual(events[0]['result'], 'Cancel' if key in cancelled else 'Success')
            source = lambda stage: next(o for o in value[stage] if o['source'] == 0)
            changed = source('before')['crc'] != source('after')['crc']
            self.assertEqual(changed, key not in no_change, key)
            self.assertEqual(source('after_script')['subd'], source('redo')['subd'])
            self.assertEqual(source('before')['subd'], source('undo')['subd'])
            witnesses = [o for o in value['undo'] if o['name'] == 'Undo witness']
            self.assertEqual(len(witnesses), int(changed), key)

    def test_fixed_validation_and_isolated_helper_imports(self):
        query = probe.request()
        for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different'),
                     dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                probe.validate(bad['operations'][0])
            if 'op' not in edit:
                with self.assertRaises(ValueError):
                    validate_operations(bad)
        modules = ['soft_crease_probe', 'soft_crease_cases', 'subd_edit_scene',
                   'subdivide_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe']
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                             {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules),
                            directory], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()
