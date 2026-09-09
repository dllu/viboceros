"""Analytic accepted-point checks, independent of command/history code."""
import json
from pathlib import Path
import unittest


class PointsMeasurements(unittest.TestCase):
    def test_accepted_points_after_enter_cancel_and_session_undo(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/points_command.json").read_text())
        response = json.loads((root / "observations/points_command.json").read_text())
        expected = {
            "points-duplicates": [[1,2,3],[4,5,6],[1,2,3]],
            "points-session-undo": [[1,2,3],[7,8,9]],
            "points-undo-all": [], "points-empty": [],
            "points-cancel": [[1,2,3],[4,5,6]],
            "points-cancel-empty": [], "points-undo-empty": [[1,2,3]],
        }
        self.assertEqual(response["engine"],"rhino")
        self.assertEqual(len(response["results"]),len(expected))
        self.assertEqual({o["id"] for o in request["operations"]},set(expected))
        for result in response["results"]:
            with self.subTest(case=result["id"]):
                self.assertEqual(result["value"]["after"], [dict(point=p,selected=False) for p in expected[result["id"]]])


if __name__ == "__main__":
    unittest.main()
