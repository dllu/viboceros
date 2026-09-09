"""Independent checks of the measured Rhino Diagonal grid locations and order."""
import json
from pathlib import Path
import unittest


class PointGridDiagonalMeasurements(unittest.TestCase):
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
