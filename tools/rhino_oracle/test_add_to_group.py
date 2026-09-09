"""Independent expectations for the named-target AddToGroup observations."""
import json
from pathlib import Path
import unittest


class AddToGroupObservationTests(unittest.TestCase):
    def test_target_clicks_choose_last_membership_without_reordering_existing_members(self):
        data = json.loads((Path(__file__).parent / "observations/add_to_group_picking.json").read_text())
        expected = {
            "ordinary-target": [[0], [0], [0]],
            "overlap-last-target": [[0, 1], [0, 1], [1]],
            "reversed-target": [[0], [1, 0], [1, 0]],
            "existing-target-member": [[0], [0, 1], [1]],
        }
        self.assertEqual({row["id"] for row in data["results"]}, set(expected))
        for row in data["results"]:
            self.assertEqual(row["value"]["memberships"], expected[row["id"]])
            self.assertEqual(row["value"]["selected"], [])

    def test_memberships_and_selection_match_literal_expectations(self):
        root = Path(__file__).parent
        data = json.loads((root / "observations/add_to_group.json").read_text())
        expected = {
            "partial-and-repeat": [["Group-0"], ["Group-1", "Group-0"], ["Group-1"]],
            "existing-members": [["Group-0"], ["Group-0", "Group-1"], ["Group-1"]],
            "empty-target": [["Group-1", "Group-0"], ["Group-0"]],
        }
        self.assertEqual({row["id"] for row in data["results"]}, set(expected))
        for row in data["results"]:
            states = row["value"]["states"]
            objects = states[-1]["objects"]
            self.assertEqual([obj["groups"] for obj in objects], expected[row["id"]])
            self.assertTrue(all(not obj["selected"] for obj in objects))
            self.assertEqual([obj["source"] for obj in objects], list(range(len(objects))))
            self.assertTrue(all(obj["retained"] for obj in objects))
            if row["id"] == "partial-and-repeat":
                self.assertEqual(states[2], states[4])
