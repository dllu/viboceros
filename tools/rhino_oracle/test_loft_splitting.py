"""Captured Loft recipes, provenance, and strict model-state comparison."""
import copy
import gzip
import hashlib
import json
import unittest
from pathlib import Path
from .client import OracleProtocolError, compare_responses
from .loft_splitting_cases import request, diagnostics, reorder_request, angle_request, manual_diagnostics, seam_diagnostics, basis_diagnostics, simplified_diagnostics, reorder_inspection, tangent_jets, chart_diagnostics, domain_holdouts, domain_compatibility
from .loft_splitting_cases import (
    domain_preparation, domain_matching, domain_aspect_holdouts,
    domain_geometric_matching, domain_fastpath_holdouts, domain_speed_samples,
    domain_speed_jets,
    domain_quadrature, open_basis_holdouts,
    orientation_holdouts, orientation_witnesses, closed_orientation_regular,
)
from .loft_splitting_comparison import canonical
from .loft_splitting_probe import validate
from .report_surface_apex_native import load_capture

HERE = Path(__file__).resolve().parent


class LoftSplittingTests(unittest.TestCase):
    def test_fresh_threshold_recheck_matches_the_frozen_model_states(self):
        query = angle_request()
        fresh = json.loads(gzip.decompress(
            (HERE/'observations/loft_splitting_angles_recheck_20261102.json.gz').read_bytes()))
        original = json.loads((HERE/'observations/loft_splitting_angles.json').read_text())
        self.assertEqual(fresh['provenance']['request_sha256'], hashlib.sha256(
            (HERE/'fixtures/loft_splitting_angles.json').read_bytes()).hexdigest())
        for sha in fresh['provenance']['sources'].values():
            archive = HERE/'observations/source_snapshots'/(sha+'.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), sha)
        self.assertEqual([r['id'] for r in fresh['results']], [o['id'] for o in query['operations']])
        self.assertEqual(fresh['engine_version'], original['engine_version'])
        local = copy.deepcopy(original)
        local['engine'] = 'viboceros'; local.pop('provenance')
        for op, row in zip(query['operations'], local['results']):
            row['value'] = canonical(op, row['value'], True)
        self.assertTrue(compare_responses(local, fresh, absolute_epsilon=1e-12,
                                         relative_epsilon=0., request=query).passed)

    def test_all_native_recipes_and_archived_producers_are_complete(self):
        total = 0
        for family, recipe, count in [
            ('loft_splitting', request, 220),
            ('loft_splitting_smoke', lambda: request(True), 4),
            ('loft_splitting_diagnostics', diagnostics, 15),
            ('loft_splitting_reorder', reorder_request, 24),
            ('loft_splitting_reorder_inspection', reorder_inspection, 24),
            ('loft_splitting_angles', angle_request, 32),
            ('loft_splitting_tangent_jets', tangent_jets, 32),
            ('loft_splitting_chart_diagnostics', chart_diagnostics, 20),
            ('loft_splitting_orientation_holdouts', orientation_holdouts, 20),
            ('loft_splitting_orientation_witnesses', orientation_witnesses, 9),
            ('loft_splitting_closed_orientation_regular', closed_orientation_regular, 8),
            ('loft_splitting_domain_holdouts', domain_holdouts, 8),
            ('loft_splitting_domain_compatibility', domain_compatibility, 8),
            ('loft_splitting_domain_preparation', domain_preparation, 8),
            ('loft_splitting_domain_matching', domain_matching, 8),
            ('loft_splitting_domain_aspect_holdouts', domain_aspect_holdouts, 8),
            ('loft_splitting_domain_geometric_matching', domain_geometric_matching, 16),
            ('loft_splitting_domain_fastpath_holdouts', domain_fastpath_holdouts, 4),
            ('loft_splitting_domain_speed_samples', domain_speed_samples, 4),
            ('loft_splitting_domain_speed_jets', domain_speed_jets, 4),
            ('loft_splitting_domain_quadrature', domain_quadrature, 4),
            ('loft_splitting_open_basis_holdouts', open_basis_holdouts, 4),
            ('loft_splitting_manual', manual_diagnostics, 4),
            ('loft_splitting_seam_diagnostics', seam_diagnostics, 6),
            ('loft_splitting_basis_diagnostics', basis_diagnostics, 10),
            ('loft_splitting_simplified_diagnostics', simplified_diagnostics, 6),
        ]:
            data, paths = load_capture(family, family, recipe)
            self.assertEqual(len(data['results']), count)
            self.assertTrue(data['engine_version'])
            self.assertTrue(any('source_snapshots' in p for p in paths))
            for op in recipe()['operations']:
                if op['op'] == 'loft_splitting': validate(op)
            total += count
        self.assertEqual(total, 510)

    def test_straight_chart_holdouts_rule_out_degree_and_section_count_policies(self):
        data = json.loads((HERE/'observations/loft_splitting_orientation_holdouts.json').read_text())
        positive = {
            'square-peak', 'square-valley', 'square-four-midplane',
            'square-five-midplane', 'square-centered', 'circle-peak', 'circle-valley',
        }
        observed = set()
        for row in data['results']:
            name = row['id'].removeprefix('loft-orientation-')
            if name.startswith('closed-'): continue
            faces = row['value']['after'][-1]['faces']
            if all(face['domain_v'][0] >= 0. for face in faces): observed.add(name)
        self.assertEqual(observed, positive)

    def test_closest_point_success_does_not_determine_straight_chart_sense(self):
        data = json.loads((HERE/'observations/loft_splitting_orientation_witnesses.json').read_text())
        signs = set()
        for row in data['results']:
            for face in row['value']['after'][-1]['faces']:
                self.assertIs(face['closest_center']['success'], True)
                signs.add(face['domain_v'][0] >= 0.)
        self.assertEqual(signs, {False, True})

    def test_simple_curvature_breaks_retain_open_spline_correspondence(self):
        rows = json.loads((HERE/'observations/loft_splitting_open_basis_holdouts.json').read_text())['results']
        for row in rows:
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            cubic = row['id'].endswith('-cubic')
            self.assertEqual(value['surfaces'][0]['control_count'], [2, 6 if cubic else 5])
            self.assertTrue(all(not c['parameters'] if cubic else c['parameters'] for c in value['geometric_matching']))
            if row['id'].endswith(('-unit', '-translated-domain', '-cubic')):
                surface = value['surfaces'][0]; a, b = surface['domain_v']
                knots = sorted(set(round((t-a)/(b-a), 12) for t in surface['knots_v']))
                self.assertEqual(knots, [0., .3, .7, 1.])

    def test_public_compatible_curves_expose_shifted_loft_section_preparation(self):
        data = json.loads((HERE/'observations/loft_splitting_domain_compatibility.json').read_text())
        count = 0
        for row in data['results']:
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            curves = value['compatible']
            self.assertEqual(len(curves), 2)
            self.assertEqual(curves[0]['knots'], curves[1]['knots'])
            if not row['id'].endswith('-1-compatible'): continue
            surface = value['surfaces'][0]
            self.assertEqual(surface['control_count'], [2, len(curves[0]['control_points'])])
            a, b = curves[0]['domain']; u, v = surface['domain_v']
            for k, t in zip(curves[0]['knots'], surface['knots_v']):
                self.assertAlmostEqual((k-a)/(b-a), (t-u)/(v-u), places=13)
            for index, curve in enumerate(curves):
                for cp, prepared in zip(surface['control_points'][index::2], curve['control_points']):
                    for a, b in zip(cp['point']+[cp['weight']], prepared['point']+[prepared['weight']]):
                        self.assertAlmostEqual(a, b, places=12)
            count += 1
        self.assertEqual(count, 4)

    def test_straight_chart_sense_depends_on_section_geometry(self):
        data = json.loads((HERE/'observations/loft_splitting_chart_diagnostics.json').read_text())
        for row in data['results']:
            label = row['id'].removeprefix('loft-chart-').rsplit('-', 1)[0]
            faces = row['value']['after'][-1]['faces']
            positive = label not in ['monotone', 'height', 'four']
            self.assertEqual(all(f['domain_v'][0] >= 0. for f in faces), positive)

    def test_translating_only_one_domain_changes_the_native_shared_chart(self):
        data = json.loads((HERE/'observations/loft_splitting_basis_diagnostics.json').read_text())
        rows = {r['id']: r['value'] for r in data['results'] if r['id'].startswith('loft-basis')}
        for label in ['none', '0', '3', '0123']:
            value = rows['loft-basis-shift-'+label]
            self.assertEqual(value['before'], value['after'])
            surface = value['surfaces'][0]
            self.assertEqual(surface['control_count'], [6, 13])
            # This measured chart is close to, but differs from, exact arc-length
            # spacing. Do not replace the native observation with that assumption.
            first = 2.639440376696249 if label in ['0','3'] else 4.
            self.assertAlmostEqual(surface['knots_v'][3], first, places=12)

    def test_near_threshold_records_show_independent_policies(self):
        data = json.loads((HERE/'observations/loft_splitting_angles.json').read_text())
        counts = {r['id']: r['value']['after'][-1]['face_count'] for r in data['results']}
        for index in range(4):
            self.assertEqual(counts['loft-angle-{}-normal-00'.format(index)], 1)
            self.assertEqual(counts['loft-angle-{}-normal-01'.format(index)], 4)
            self.assertEqual(counts['loft-angle-{}-normal-10'.format(index)], 1 if index < 2 else 2)
            self.assertEqual(counts['loft-angle-{}-straight-10'.format(index)], 3 if index < 2 else 6)

    def test_all_276_core_states_have_a_lossless_model_projection(self):
        for family in ['loft_splitting', 'loft_splitting_reorder', 'loft_splitting_angles']:
            query = json.loads((HERE/'fixtures'/(family+'.json')).read_text())
            data = json.loads((HERE/'observations'/(family+'.json')).read_text())
            for op, row in zip(query['operations'], data['results']):
                projected = canonical(op, row['value'], True)
                self.assertEqual(canonical(op, projected, False), projected)
                self.assertEqual(projected['after'][-1]['faces'], row['value']['after'][-1]['faces'])

    def test_malformed_inputs_topology_and_traces_are_rejected(self):
        op = request(True)['operations'][0]
        for key in ['closed', 'crease_splitting', 'split_at_tangents', 'preselect',
                    'natural', 'inspection', 'orientation_inspection', 'orientation_witness']:
            invalid = copy.deepcopy(op); invalid[key] = 1
            with self.subTest(key=key), self.assertRaises(ValueError): validate(invalid)
        invalid = copy.deepcopy(op); invalid['curves'][0]['degree'] = True
        with self.assertRaises(ValueError): validate(invalid)
        query = request()
        data = json.loads((HERE/'observations/loft_splitting.json').read_text())
        op = query['operations'][0]; value = data['results'][0]['value']
        for key, replacement in [('face_count', True), ('vertex_count', 0), ('is_solid', 1)]:
            invalid = copy.deepcopy(value); invalid['after'][-1][key] = replacement
            with self.subTest(key=key), self.assertRaises(OracleProtocolError): canonical(op, invalid, True)
        invalid = copy.deepcopy(value); invalid['events'] *= 2
        with self.assertRaises(OracleProtocolError): canonical(op, invalid, True)
        invalid = copy.deepcopy(value); invalid['after'][-1]['faces'][0]['samples'][0][0] = float('nan')
        with self.assertRaises(OracleProtocolError): canonical(op, invalid, True)

    def test_public_comparison_retains_surface_and_history_differences(self):
        query = request(True)
        native = json.loads((HERE/'observations/loft_splitting_smoke.json').read_text())
        local = copy.deepcopy(native); local['engine'] = 'viboceros'; local.pop('provenance')
        for op, row in zip(query['operations'], local['results']): row['value'] = canonical(op, row['value'], True)
        self.assertTrue(compare_responses(local, native, absolute_epsilon=1e-6, relative_epsilon=0., request=query).passed)
        local['results'][0]['value']['redo'][-1]['faces'][0]['samples'][0][0] += .01
        self.assertFalse(compare_responses(local, native, absolute_epsilon=1e-6, relative_epsilon=0., request=query).passed)


if __name__ == '__main__': unittest.main()
