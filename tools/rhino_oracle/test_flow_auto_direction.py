import copy
import json
from pathlib import Path
from unittest import TestCase, skipIf

try:
    import numpy as np
except ModuleNotFoundError:
    np = None
if np is not None:
    from .flow_auto_direction_report import ROTATIONS, comparison, report

BASE = Path(__file__).parent


@skipIf(np is None, 'optional automatic Flow analysis requires NumPy')
class FlowAutoDirectionTests(TestCase):
    @classmethod
    def setUpClass(cls):
        def read(path):
            return json.loads((BASE/path).read_text())
        cls.request = read('fixtures/flow_auto_directions.json')
        cls.local = read('observations/flow_auto_directions_viboceros_20261017.json')
        cls.native = read('observations/flow_auto_directions_rhino_20261017.json')

    def test_retained_report_is_reproducible_and_does_not_claim_orientation_parity(self):
        actual = report()
        expected = json.loads((BASE/'observations/flow_auto_direction_report_20261017.json').read_text())
        def close(a, b):
            if isinstance(a, dict):
                return a.keys() == b.keys() and all(close(a[k], b[k]) for k in a)
            if isinstance(a, list):
                return len(a) == len(b) and all(close(x, y) for x, y in zip(a, b))
            if isinstance(a, float):
                return abs(a-b) <= 1e-10
            return a == b
        self.assertTrue(close(actual, expected))
        self.assertEqual(actual['cases'], 38)
        self.assertEqual(actual['native_basis_sections'], 646)
        self.assertLess(actual['max_shared_direction_basis_error'], 1e-10)
        self.assertLess(actual['max_kernel_center_coordinate_error'], 1e-10)
        for row in actual['results']:
            self.assertEqual(row['training_sections'], 9)
            self.assertEqual(row['held_out_sections'], 8)
            self.assertGreater(row['kernel_basis_max_coordinate_error'], .01)
        self.assertFalse(actual['production_kernel_changed'])
        self.assertFalse(actual['automatic_spatial_flow_qualified'])
        self.assertFalse(actual['fitted_directions_used_as_kernel_inputs'])

    def test_translations_source_length_and_orientation_do_not_choose_the_target_pole(self):
        r = report()
        self.assertEqual(len(r['translation_and_source_length_invariants']), 14)
        self.assertLess(r['max_translation_and_source_length_direction_error'], 1e-10)
        for row in r['rotation_comparisons']:
            self.assertLess(row['changed_source_orientation_direction_error'], 1e-10)
            self.assertGreater(row['joint_rotation_direction_error'], .003)
        self.assertGreater(r['target_reversal_direction_error'], .1)
        self.assertGreater(r['exact_refinement_direction_error'], 1e-6)
        for row in r['scale_classification']:
            if row['public_plane_at_0_1_available']:
                self.assertGreater(row['direction_error_from_baseline'], .5)
            else:
                self.assertLess(row['direction_error_from_baseline'], 1e-10)
        small = [r for r in r['results'] if r['id'].startswith(('both-scale-0-01', 'target-scale-0-01'))]
        self.assertEqual(len(small), 2)
        for row in small:
            # These bases have a vanishing b coefficient in the homogeneous
            # fit. The stronger k coefficient still recovers the shared pole.
            self.assertLess(row['direction_error_from_public_plane'], 1e-10)
            self.assertLess(abs(row['fitted_direction'][1]-1), 1e-10)

    def test_quarter_turn_witnesses_rotate_authored_guides_and_probe_bases(self):
        seed = self.request['operations'][0]
        for op in self.request['operations'][-8:]:
            name, roles = op['id'].rsplit('-', 1)
            matrix = np.asarray(ROTATIONS[name])
            points = lambda curve: np.asarray([c['point'] for c in curve['control_points']])
            np.testing.assert_array_equal(points(op['target']), points(seed['target']) @ matrix.T)
            for field in ['start', 'end']:
                expected = np.asarray(seed['base'][field])
                np.testing.assert_array_equal(op['base'][field], expected @ matrix.T if roles == 'both' else expected)
            expected = np.asarray(seed['points'])
            np.testing.assert_array_equal(op['points'], expected @ matrix.T if roles == 'both' else expected)

    def test_bad_identity_nonfinite_values_and_incomplete_bases_fail(self):
        for mutate in [
            lambda r: r['results'].reverse(),
            lambda r: r['results'][0]['value']['sdk_points'].pop(),
            lambda r: r['results'][0]['value']['sdk_points'][1].__setitem__(0, float('nan')),
            lambda r: r['results'][0]['value']['sdk_points'][1].__setitem__(0, float('inf')),
            lambda r: r['results'][0]['value']['sdk_points'][1].__setitem__(0, 10.),
        ]:
            native = copy.deepcopy(self.native)
            mutate(native)
            with self.assertRaises(ValueError):
                comparison(self.request, self.local, native)

    def test_valid_rotation_of_an_unfitted_basis_and_finite_local_corruption_remain_visible(self):
        native = copy.deepcopy(self.native)
        p = native['results'][0]['value']['sdk_points']
        center, y, z = map(np.asarray, p[3:6])
        y, z = y-center, z-center
        p[4] = (center + np.cos(.1)*y + np.sin(.1)*z).tolist()
        p[5] = (center - np.sin(.1)*y + np.cos(.1)*z).tolist()
        r = comparison(self.request, self.local, native)
        self.assertGreater(r['results'][0]['max_shared_direction_basis_error'], .01)
        local = copy.deepcopy(self.local)
        local['results'][0]['value']['sdk_points'][0][0] += .01
        r = comparison(self.request, local, self.native)
        self.assertGreater(r['max_kernel_center_coordinate_error'], .009)
