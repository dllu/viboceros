"""Crossing capture closure, immutable provenance and explicit compatibility gaps."""
import copy
import gzip
import hashlib
import json
import unittest
from pathlib import Path
from . import mesh_patch_probe as probe
from .report_mesh_patch_crossings import GROUPS, summarize
from .structural_sdk import validate_operations

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def capture(suffix):
    return json.loads((HERE / f'observations/mesh_patch_crossings{suffix}.json').read_text())


def canonical(mesh):
    vertices = [tuple(p) for p in mesh['vertices']]
    return sorted(vertices), sorted(tuple(sorted(vertices[i] for i in face)) for face in mesh['faces'])


class MeshPatchCrossingTests(unittest.TestCase):
    def test_fixed_capture_requests_sources_and_history(self):
        factories = [probe.crossing_request, probe.crossing_verify_request,
                     probe.untrimmed_crossing_request, probe.seeded_crossing_request]
        for (_, _, suffix), factory, count in zip(GROUPS, factories, [11, 21, 9, 7]):
            path = HERE / f'fixtures/mesh_patch_crossings{suffix}.json'
            request = json.loads(path.read_text())
            self.assertEqual(request, factory())
            validate_operations(request)
            response = capture(suffix)
            self.assertEqual(len(response['results']), count)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), response['provenance']['request_sha256'])
            for name, sha in response['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            for op, row in zip(request['operations'], response['results']):
                self.assertEqual(op['id'], row['id'])
                self.assertNotIn('error', row)
                v = row['value']
                self.assertEqual(v['input'], probe.SCENES[op['id']])
                events = [e for e in v['events'] if e['name'] == 'MeshPatch']
                self.assertEqual(len(events), 1)
                self.assertEqual(events[0]['result'], 'Success')
                parents = [dict(o, selected=False) for o in v['before'] if o['role'] != 'output']
                for phase in ['after_script', 'undo', 'redo']:
                    self.assertEqual([o for o in v[phase] if o['role'] != 'output'], parents)
                    outputs = [o for o in v[phase] if o['role'] == 'output']
                    self.assertEqual(len(outputs), int(phase != 'undo'))
                    for output in outputs:
                        self.assertEqual(output['layer'], 'MeshPatch output')
                        self.assertFalse(output['selected'])
                        self.assertIsNone(output['name'])
            request['operations'][0]['id'] = 'changed'
            self.assertNotEqual(request, factory())

    def test_independent_owned_processes_repeat_both_native_meshes(self):
        original = capture('')['results']
        repeat = capture('_verify')['results'][:len(original)]
        for first, second in zip(original, repeat):
            self.assertEqual(first['id'], second['id'])
            for row in [first, second]:
                row['value']['command_mesh'] = next(o['mesh'] for o in row['value']['after_script'] if o['role'] == 'output')
            for key in ['sdk', 'command_mesh']:
                self.assertEqual(canonical(first['value'][key]), canonical(second['value'][key]), (first['id'], key))

    def test_only_named_scenes_can_reach_the_probe(self):
        for factory in [probe.crossing_request, probe.crossing_verify_request,
                        probe.untrimmed_crossing_request, probe.seeded_crossing_request]:
            for change in [{'case': 'arbitrary'}, {'id': 'arbitrary'}, {'script': 'arbitrary'}, {'points': [[0, 0, 0]]}]:
                q = factory()
                q['operations'][0].update(change)
                with self.assertRaises(ValueError):
                    validate_operations(q)

    def report_lines(self):
        report = json.loads((ROOT / 'docs/mesh-patch-crossings-qualification.json').read_text())
        lines = [f'MESH_PATCH_{prefix}_{stage.upper()} ' + json.dumps(row)
                 for name, prefix, _ in GROUPS for stage in ['geometry', 'command', 'getter']
                 for row in report['groups'][name][stage]['cases']]
        return report, lines

    def test_report_retains_every_scene_sdk_difference_and_current_source(self):
        report, lines = self.report_lines()
        self.assertEqual(summarize('\n'.join(lines)), report)
        self.assertEqual(report['model_epsilon'], 1e-6)
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertEqual(report['captures'], 48)
        self.assertEqual(report['unique_scenes'], 37)
        self.assertEqual(report['qualified_unique_scenes'], 31)
        self.assertEqual(set(report['unqualified_scenes']), {'shared_endpoint', 'overlap', 'self_cross', 'cross_h_reverse', 'two_crossings', 'cross_seed_axis'})
        first = report['groups']['crossings']['geometry']['cases'][0]
        self.assertTrue(first['qualified'])
        self.assertGreater(first['sdk_command_difference'], report['model_epsilon'])
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_report_rejects_omissions_duplicates_nonfinite_values_and_hidden_gaps(self):
        report, lines = self.report_lines()
        for invalid in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        for error in [1e-6, -1., float('nan'), float('inf'), True]:
            row = copy.deepcopy(report['groups']['crossings']['geometry']['cases'][0])
            row['maximum_bidirectional_sample_error'] = error
            invalid = lines[:]
            invalid[0] = 'MESH_PATCH_CROSSING_GEOMETRY ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        for field in ['counts_match', 'source_metadata_preserved', 'grouped_history']:
            invalid = lines[:]
            index = next(i for i, line in enumerate(invalid) if line.startswith('MESH_PATCH_CROSSING_COMMAND '))
            row = json.loads(invalid[index].split(' ', 1)[1])
            row[field] = False
            invalid[index] = 'MESH_PATCH_CROSSING_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(invalid))
        invalid = lines[:]
        index = next(i for i, line in enumerate(invalid) if line.startswith('MESH_PATCH_CROSSING_GEOMETRY ') and not json.loads(line.split(' ', 1)[1])['qualified'])
        row = json.loads(invalid[index].split(' ', 1)[1])
        row['qualified'] = True
        invalid[index] = 'MESH_PATCH_CROSSING_GEOMETRY ' + json.dumps(row)
        with self.assertRaises(ValueError):
            summarize('\n'.join(invalid))
