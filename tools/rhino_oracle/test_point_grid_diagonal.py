"""Independent checks of the measured Rhino Diagonal grid locations and order."""
import json
from pathlib import Path
import unittest


class PointGridDiagonalMeasurements(unittest.TestCase):
    def test_oriented_planes_match_analytic_directed_lattices(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/point_matrix_diagonal_planes.json").read_text())
        response = json.loads((root / "observations/point_matrix_diagonal_planes.json").read_text())
        # Independent world-coordinate formulas, not the production frame or
        # oracle sorting routines. i, j, k are normalized lattice stations.
        formulas = {
            "diagonal-xz": lambda i, j, k: [1 + 6*i, 2 - 4*k, 3 + 5*j],
            "diagonal-yz": lambda i, j, k: [1 - 4*k, 2 - 6*i, 3 + 5*j],
            "diagonal-oblique": lambda i, j, k: [1 + 3.6*i + 3.2*k, 2 + 4.8*i - 2.4*k, 3 + 5*j],
            "diagonal-xz-planar": lambda i, j, k: [1 - 6*i, 2 + 4*k, 3 + 5*j],
        }
        self.assertEqual(response["engine"], "rhino")
        self.assertEqual(len(response["results"]), len(formulas))
        self.assertEqual({op["id"] for op in request["operations"]}, set(formulas))
        self.assertEqual({result["id"] for result in response["results"]}, set(formulas))
        for result in response["results"]:
            formula = formulas[result["id"]]
            expected = [formula(i, j, k) for k in (0, 1) for j in (0, 1) for i in (0, 0.5, 1)]
            actual = result["value"]["points"]
            with self.subTest(case=result["id"]):
                self.assertEqual(len(actual), len(expected))
                for p, q in zip(actual, expected):
                    self.assertEqual(len(p), 3)
                    for a, b in zip(p, q):
                        self.assertLessEqual(abs(a - b), 1e-12)

    def test_recorded_diagonal_grids_match_directed_rectangular_lattices(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/point_matrix_diagonal.json").read_text())
        response = json.loads((root / "observations/point_matrix_diagonal_prompt.json").read_text())
        axes = {
            "diagonal-positive": ([10, 11, 12], [20, 25], [3, 5, 7]),
            "diagonal-negative": ([10, 9, 8], [20, 24], [3, -1]),
            "diagonal-planar": ([0, 3, 6], [0, 4], [0, -2]),
            "diagonal-planar-elevated": ([0, 3, 6], [0, 4], [3, 5]),
        }
        self.assertEqual(response["engine"], "rhino")
        self.assertEqual(len(response["results"]), len(axes))
        self.assertEqual({op["id"] for op in request["operations"]}, set(axes))
        self.assertEqual({result["id"] for result in response["results"]}, set(axes))
        for result in response["results"]:
            x_axis, y_axis, z_axis = axes[result["id"]]
            expected = [[x, y, z] for z in z_axis for y in y_axis for x in x_axis]
            with self.subTest(case=result["id"]):
                self.assertEqual(result["value"]["points"], expected)


if __name__ == "__main__":
    unittest.main()
