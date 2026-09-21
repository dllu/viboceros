"""Independent exact reference invariants; no native or Rhino result inputs."""
from fractions import Fraction as F
from pathlib import Path
import unittest
from .references import projected_lines as ref


class ProjectedLineTests(unittest.TestCase):
    def test_known_perspective_line(self):
        matrix = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]]
        self.assertEqual(ref.closest(matrix, [0, 0, 1], [2, 0, 2],
                                    [F(1, 2), F(1, 10)], F(1, 8)),
                         ([F(2, 3), F(0), F(4, 3)], F(1, 100)))

    def test_clipping_boundary_and_invisible_line(self):
        matrix = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]]
        self.assertEqual(ref.closest(matrix, [0, 0, 1], [1, 0, -1], [8, 0], F(1, 2)),
                         ([F(1, 4), F(0), F(1, 2)], F(225, 4)))
        self.assertIsNone(ref.closest(matrix, [0, 0, -1], [1, 0, -2], [0, 0], 1))

    def test_corpus_matches_generated_input_and_exact_stationarity(self):
        root = Path(__file__).resolve().parents[2]
        path = root / "crates/viboceros-drafting/src/object_snap/projected_line/reference.csv"
        self.assertEqual(path.read_text(), ref.csv_text())
        rows = list(ref.cases())
        self.assertEqual(len(rows), 288)
        for row in rows:
            matrix = [row[i:i+4] for i in range(0, 12, 4)]
            a, b, cursor, near = row[12:15], row[15:18], row[18:20], row[20]
            point, squared = ref.closest(matrix, a, b, cursor, near)
            self.assertEqual(ref.closest(matrix, b, a, cursor, near), (point, squared))
            h = ref.homogeneous(matrix, point)
            self.assertGreaterEqual(h[2], F(near))
            self.assertEqual(sum((h[i] / h[2] - F(cursor[i]))**2 for i in range(2)), squared)
            ha, hb = ref.homogeneous(matrix, a), ref.homogeneous(matrix, b)
            derivative = sum((h[i] / h[2] - F(cursor[i])) * (
                (hb[i] - ha[i]) * h[2] - h[i] * (hb[2] - ha[2])) for i in range(2))
            if point != list(map(F, a)) and point != list(map(F, b)) and h[2] != F(near):
                self.assertEqual(derivative, 0)
            elif point == list(map(F, a)) or (h[2] == F(near) and ha[2] < F(near)):
                self.assertGreaterEqual(derivative, 0)
            else:
                self.assertLessEqual(derivative, 0)
