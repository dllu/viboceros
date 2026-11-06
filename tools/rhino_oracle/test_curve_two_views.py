"""Fixed two-view evidence, immutable producers and source/history guards."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from . import curve_two_views_cases as cases
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent
FAMILIES = [('', cases.cases, 29), ('_discovery', cases.discovery, 5),
            ('_audit', cases.audit_cases, 5), ('_fit_audit', cases.fit_audit_cases, 3),
            ('_surface_audit', cases.surface_audit_cases, 3)]


def observation(suffix):
    path = HERE / ('observations/curve_two_views' + suffix + '.json')
    return json.loads(path.read_bytes() if path.exists() else
                      gzip.decompress(path.with_suffix('.json.gz').read_bytes()))


class TwoViewEvidenceTests(unittest.TestCase):
    def test_validation_record_preserves_unqualified_geometry_and_all_source_hashes(self):
        from .report_curve_two_views import ROOT, build
        record = json.loads((ROOT / 'docs/curve-two-views-validation.json').read_text())
        self.assertEqual(record, build(ROOT / 'docs/curve-two-views-proof'))
        self.assertFalse(record['command_registered'])
        self.assertFalse(record['full_native_parity'])
        self.assertEqual(record['model_epsilon'], 1e-6)
        self.assertEqual(record['above_gate_sampled_upper_bounds'], 19)
        self.assertEqual(record['output_count_gaps'], 1)

    def test_fixed_requests_and_all_frozen_producers(self):
        for suffix, factory, count in FAMILIES:
            request = (HERE / ('fixtures/curve_two_views' + suffix + '.json')).read_bytes()
            query = json.loads(request)
            self.assertEqual(query['operations'], factory())
            validate_operations(query)
            native = observation(suffix)
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in query['operations']])
            self.assertTrue(all('error' not in r for r in native['results']))
            provenance = native['provenance']
            self.assertEqual(provenance['request_sha256'], hashlib.sha256(request).hexdigest())
            for filename, digest in provenance['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, filename)

    def test_recipe_identity_ignores_key_order_and_rejects_value_or_type_changes(self):
        source = cases.cases()[0]
        cases.validate(dict(reversed(list(source.items()))))
        for mutate in [lambda r: r.update(id='foreign'), lambda r: r.update(extra=True),
                       lambda r: r['directions'][0].__setitem__(0, True),
                       lambda r: r['curves'][0]['control_points'][0].update(weight=float('nan')),
                       lambda r: r.update(mode='arbitrary')]:
            row = copy.deepcopy(source)
            mutate(row)
            with self.assertRaisesRegex(ValueError, 'fixed owned'):
                cases.validate(row)

    def test_command_parents_output_attributes_and_native_only_cancellations(self):
        rows = observation('')['results']
        completed = cancelled = 0
        for op, row in zip(cases.cases(), rows, strict=True):
            v = row['value']
            if op.get('cancel'):
                cancelled += 1
                self.assertFalse(v['success'])
                self.assertEqual(v['before'], v['after'])
                continue
            originals = {o['source']: o for o in v['before']['objects']}
            for phase in ['after', 'undo', 'redo']:
                for obj in v[phase]['objects']:
                    if obj['source'] is not None:
                        a, b = dict(obj), dict(originals[obj['source']])
                        a.pop('selected'); b.pop('selected')
                        self.assertEqual(a, b)
                    else:
                        self.assertFalse(obj['selected'])
                        self.assertTrue(obj['native_valid'])
                        self.assertEqual(obj['geometry_user_text'], {})
                        self.assertEqual(obj['attributes']['layer'], 'Crv2View Output')
                        for key in ['name', 'groups', 'user_text']:
                            self.assertFalse(obj['attributes'][key])
            if v['success']:
                completed += 1
                self.assertEqual(v['after'], v['redo'])
                self.assertTrue(all(o['source'] is not None for o in v['undo']['objects']))
            else:
                self.assertEqual(v['before'], v['after'])
        self.assertEqual((completed, cancelled), (21, 6))

    def test_sdk_controls_retain_unchanged_documents_and_parallel_pathology(self):
        discovery = {r['id'].removeprefix('two_views_discovery_'): r['value']
                     for r in observation('_discovery')['results']}
        for mode in ['sdk', 'auto', 'before']:
            self.assertTrue(discovery[mode]['success'])
        self.assertFalse(discovery['after']['success'])
        self.assertEqual(discovery['sdk']['before'], discovery['sdk']['after'])
        self.assertGreater(discovery['parallel']['output'][0]['length'], 1e8)
        for row in observation('_audit')['results']:
            v = row['value']
            self.assertEqual(v['before'], v['after'])
            self.assertEqual(v['audit']['samples_per_span'], 9)
            self.assertGreater(v['audit']['maximum_known_closest_distance'], 1e-6)
        for row in observation('_fit_audit')['results']:
            self.assertTrue(row['value']['input_unchanged'])

    def test_staged_helper_closure(self):
        query = dict(operations=cases.cases())
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            for name in ['curve_two_views_cases', 'curve_two_views_probe', 'curve_two_views_fit_probe',
                         'curve_two_views_surface_probe', 'planar_regions_cases', 'join_probe', 'merge_edges_probe']:
                self.assertEqual((Path(directory) / (name + '.py')).read_bytes(), (HERE / (name + '.py')).read_bytes())


if __name__ == '__main__':
    unittest.main()
