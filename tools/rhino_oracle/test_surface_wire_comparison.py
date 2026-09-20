from copy import deepcopy
from unittest import TestCase
from tools.numerics.compare_surface_wire_records import align_wires


class SurfaceWireComparisonTests(TestCase):
    def test_bijective_permutation_preserves_all_coordinates(self):
        a = [[[x, t, 0.] for t in [0.,0.125,0.3,0.5,0.875,1.]] for x in [0.,1.,2.]]
        b = deepcopy([a[2],a[0],a[1]])
        b[2][2][0] += 1e-5
        original = deepcopy(b)
        aligned, order = align_wires(a,b)
        self.assertEqual(order,[1,2,0])
        self.assertEqual(aligned[1][2][0],1.00001)
        self.assertEqual(b,original)

    def test_missing_duplicate_and_nonbijective_matches_are_not_silently_accepted(self):
        a = [[0.,0.,0.]]*6
        b = [[1.,0.,0.]]*6
        for native,reference in [([a],[]),([a,a],[a,b]),([a,b],[a,a])]:
            with self.assertRaises(ValueError):
                align_wires(native,reference)

    def test_malformed_or_nonfinite_samples_cannot_be_truncated_by_zip(self):
        a = [[0.,0.,0.]]*6
        for invalid in [a[:-1],a+[[0.,0.,0.]],[[0.,0.]]*6,[[0.,0.,0.,1.]]*6,
                        [[float("nan"),0.,0.]]*6,[[True,0.,0.]]*6]:
            with self.assertRaises(ValueError):
                align_wires([a],[invalid])
