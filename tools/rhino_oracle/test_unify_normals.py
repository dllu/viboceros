"""Fixed native winding references, immutable captures and honest seed limits."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .unify_normals_cases import request, validate
from .report_unify_normals import HERE, ROOT, FAMILIES, GAPS, native, summarize
from .structural_sdk import validate_operations, stage_helpers


class UnifyNormalsTests(unittest.TestCase):
    def test_fixed_inputs_frozen_producers_and_saved_sources(self):
        _, groups = native()
        self.assertEqual((len(groups['sdk']), len(groups['command'])), (1180, 65))
        for family, (flags, _) in FAMILIES.items():
            payload = request(**flags)
            validate_operations(payload)
            query = HERE / 'fixtures' / (family + '.json')
            data = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():
                content = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest)
            for row in data['results']:
                if 'command' in row['value']['input']:
                    self.assertEqual((HERE / 'fixtures' / family / (row['id'] + '.3dm')).read_bytes(),
                                     base64.b64decode(row['value']['source_3dm'], validate=True))
        folder = HERE / 'diagnostics/unify_normals'
        failed = json.loads((folder / 'seed-staging-failure.json').read_text())
        self.assertIs(failed['qualification'], False)
        self.assertEqual(failed['log_sha256'], hashlib.sha256((folder / 'seed-staging-failure.log').read_bytes()).hexdigest())
        self.assertEqual(failed['request_sha256'], hashlib.sha256((HERE / 'fixtures/unify_normals_seeds.json').read_bytes()).hexdigest())
        for digest in failed['sources'].values():
            self.assertEqual(hashlib.sha256(gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())).hexdigest(), digest)

    def test_count_only_normals_union_seams_and_raw_ngon_cycles(self):
        rows = native()[1]['sdk']
        for v in rows.values():
            self.assertEqual(v['before'], v['after_count'])
            self.assertTrue(v['after_second']['valid'])
        v = rows['unify_extra_repeat_normals']
        self.assertEqual(v['changed'], 2)
        self.assertEqual(v['after']['normals'], [[-1., 0., 0.]] * 4 + [[1., 0., 0.]])
        self.assertEqual(v['after']['ngons'], v['before']['ngons'])
        for name, count in [('seam', 6), ('unused', 5)]:
            self.assertEqual(len(rows['unify_normals_' + name]['after']['vertices']), count)
        self.assertEqual(rows['unify_normals_nonmanifold']['changed'], 2)
        self.assertEqual(rows['unify_extra_mobius']['changed'], 4)

    def test_command_normal_rebuilding_and_changed_only_history(self):
        rows = native()[1]['command']
        v = rows['unify_command_channels']
        self.assertEqual(next(o['mesh']['vertex_normals'] for o in v['after'] if o['source'] == 0), [[0., 0., 1.]] * 4)
        v = rows['unify_command_two_noops']
        self.assertEqual([o['source'] for o in v['after']], [0, 1, None])
        self.assertEqual([o['source'] for o in v['undo']], [0, 1])
        self.assertTrue(all(o['selected'] for o in v['after'] if o['source'] is not None))
        self.assertTrue(all(not o['selected'] for o in v['undo']))
        for name, v in rows.items():
            if name.startswith('unify_followup_command_ngon') and 'noop' not in name:
                before = next(o['mesh']['ngons'][0] for o in v['before'] if o['source'] == 0)
                after = next(o['mesh']['ngons'][0] for o in v['after'] if o['source'] == 0)
                self.assertEqual(before['vertices'][0], after['vertices'][0])
                self.assertEqual(before['faces'], after['faces'])

    def test_allowlist_standalone_staging_and_recapture_refusal(self):
        payload = request(followups=True)
        for op in [None, {}, [], dict(payload['operations'][0], case='arbitrary'),
                   dict(payload['operations'][0], path='arbitrary')]:
            with self.assertRaises(ValueError):
                validate(op)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(payload, HERE, Path(directory))
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import unify_normals_probe', directory],
                           check=True, capture_output=True)
        for family, (flags, _) in FAMILIES.items():
            paths = [HERE / d / (family + '.json') for d in ['fixtures', 'observations']]
            before = [p.read_bytes() for p in paths]
            result = subprocess.run([sys.executable, '-m', 'tools.rhino_oracle.generate_unify_normals']
                                    + ['--' + flag for flag in flags], cwd=ROOT, text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('observations are immutable', result.stderr)
            self.assertEqual([p.read_bytes() for p in paths], before)

    def evidence(self):
        data = json.loads((ROOT / 'docs/unify-normals-qualification.json').read_text())
        return data, ['UNIFY_NORMALS_' + stage.upper() + ' ' + json.dumps(row)
                      for stage in ['sdk', 'command', 'getter'] for row in data[stage]]

    def test_report_reconstructs_resolves_renumbering_and_keeps_component_gaps(self):
        data, lines = self.evidence()
        self.assertEqual(reconstruct_measurement(data, summarize, '\n'.join(reversed(lines))), data)
        self.assertEqual((data['qualified_sdk_cases'], data['sdk_orientation_gaps'],
                          data['registered_command_cases'], data['getter_cases']), (1169, 11, 63, 65))
        self.assertEqual(data['native_repeat_edit_cases'], 8)
        self.assertEqual({r['id'] for r in data['sdk'] if r['qualified'] is False}, GAPS)
        self.assertTrue(next(r for r in data['sdk'] if r['id'] == 'unify_holdout_reindex_1032')['qualified'])
        self.assertTrue(all(r['qualified'] for r in data['sdk'] if r['id'].startswith('unify_followup_regions_')))
        self.assertIs(data['full_native_parity'], False)
        self.assertIs(data['performance_qualified'], False)

    def test_missing_duplicate_forged_and_relabelled_evidence_fail(self):
        data, lines = self.evidence()
        for text in ['\n'.join(lines[1:]), '\n'.join(lines + lines[:1])]:
            with self.assertRaises(ValueError):
                summarize(text)
        for stage in ['sdk', 'command', 'getter']:
            row = data[stage][0]
            prefix = 'UNIFY_NORMALS_' + stage.upper() + ' '
            remaining = [s for s in lines if s != prefix + json.dumps(row)]
            edits = [('qualified', False), ('normals', False), ('ngons', False), ('maximum_error', float('nan'))]
            edits += [('predicted', True), ('source_unchanged', False), ('repeat', False),
                      ('flipped_faces', [True]), ('flipped_faces', [0, 0]),
                      ('repeat_flipped_faces', [999])] if stage == 'sdk' else [('accepted', False), ('history', False)]
            for key, value in edits:
                bad = deepcopy(row)
                bad[key] = value
                with self.assertRaises(ValueError):
                    summarize('\n'.join(remaining + [prefix + json.dumps(bad)]))
        gap = next(r for r in data['sdk'] if r['id'] in GAPS)
        remaining = [s for s in lines if s != 'UNIFY_NORMALS_SDK ' + json.dumps(gap)]
        for key in ['geometry', 'qualified']:
            bad = dict(gap, **{key: True})
            with self.assertRaises(ValueError):
                summarize('\n'.join(remaining + ['UNIFY_NORMALS_SDK ' + json.dumps(bad)]))

    def test_fresh_and_duplicate_runtime_controls_reproduce_repeat_edits(self):
        rows = native()[1]['sdk']
        controls = [v for v in rows.values() if v['input'].get('runtime_checks')]
        self.assertEqual(len(controls), 6)
        for v in controls:
            for trial in v['runtime'].values():
                self.assertEqual(trial['changed'], v['second'])
                self.assertEqual(trial['before']['face_edges'], v['after']['face_edges'])
                self.assertEqual(trial['after']['faces'], v['after_second']['faces'])
        v = rows['unify_runtime_isolated_00']
        self.assertEqual((v['changed'], v['second']), (2, 1))
        self.assertNotEqual(v['after']['faces'], v['after_second']['faces'])


if __name__ == '__main__':
    unittest.main()
