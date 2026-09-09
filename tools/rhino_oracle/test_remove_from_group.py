"""Independent semantic checks for RemoveFromGroup's explicit Copy modes."""
import json
from pathlib import Path
import unittest


class RemoveFromGroupObservationTests(unittest.TestCase):
    def test_outputs_are_selected_and_ungrouped_while_copy_retains_originals(self):
        rows = json.loads((Path(__file__).parent / "observations/remove_from_group.json").read_text())["results"]
        self.assertEqual({row["id"] for row in rows}, {"overlap-remove", "overlap-copy", "whole-group-remove", "whole-group-copy"})
        for row in rows:
            copy = row["id"].endswith("copy")
            overlap = row["id"].startswith("overlap")
            state = row["value"]["states"][-1]
            outputs = [obj for obj in state["objects"] if obj["selected"]]
            self.assertEqual([obj["source"] for obj in outputs], [1] if overlap else [0, 1])
            self.assertTrue(all(obj["groups"] == [] and obj["retained"] != copy for obj in outputs))
            for obj in outputs:
                self.assertEqual(obj["points"], [[obj["source"], 0, 0]])
                self.assertEqual(obj["name"], str(obj["source"]))
            if copy:
                originals = [obj for obj in state["objects"] if obj["retained"]]
                self.assertEqual([obj["groups"] for obj in originals], [["Group-0"], ["Group-0", "Group-1"], ["Group-1"]] if overlap else [["Group-0"], ["Group-0"]])
                self.assertTrue(all(not obj["selected"] for obj in originals))
            elif not overlap:
                self.assertEqual(state["groups"], [{"members": [], "name": "Group-0"}])
