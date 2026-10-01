import copy
import json
from pathlib import Path
import unittest
from unittest.mock import Mock

from .untrim_cases import border_request
from .untrim_probe import validate
from .untrim_replay import replay


class UntrimBorderTests(unittest.TestCase):
    def captures(self):
        root=Path(__file__).parent
        return (json.loads((root/"fixtures/untrim_border.json").read_text()),
                json.loads((root/"observations/untrim_border.json").read_text()))

    def test_source_regeneration_and_preserved_hole_definitions(self):
        fixture,observed=self.captures()
        self.assertEqual(border_request(),fixture)
        self.assertEqual(len(fixture["operations"]),100)
        self.assertEqual(len(observed["results"]),100)
        successes=0
        for operation,row in zip(fixture["operations"],observed["results"]):
            validate(operation)
            self.assertEqual(operation["id"],row["id"])
            value=row["value"]
            if not value["succeeded"]:
                self.assertIn(operation["id"].split("-keep-")[0],("box","paraboloid-oblique"))
                for before,after in zip(value["before"],value["after"]):
                    self.assertEqual(before["geometry"],after["geometry"])
                continue
            successes+=1
            sources={obj["source"]:obj for obj in value["after"] if obj["source"] is not None}
            for before in value["before"]:
                after=sources[before["source"]]
                for key in ("name","groups","current_layer","color","color_source"):
                    self.assertEqual(before[key],after[key])
                self.assertEqual(after["selected"],operation["preselect"])
                if before["geometry"]["type"]!="brep":continue
                old,new=(obj["geometry"]["definition"] for obj in (before,after))
                self.assertEqual(old["faces"][0]["definition"],new["faces"][0]["definition"])
                self.assertEqual(old["faces"][0]["loops"][1:],new["faces"][0]["loops"][1:])
                old_face,new_face=(record["topology"]["faces"][0] for record in (old,new))
                self.assertEqual(old_face["reversed"],new_face["reversed"])
                for old_loop,new_loop in zip(old_face["loops"][1:],new_face["loops"][1:]):
                    self.assertEqual(len(old_loop["trims"]),len(new_loop["trims"]))
                    for a,b in zip(old_loop["trims"],new_loop["trims"]):
                        self.assertEqual(old["edges"][a["edge"]],new["edges"][b["edge"]])
            for obj in value["after"]:
                if obj["source"] is None:
                    self.assertTrue(operation["keep_trim_objects"])
                    self.assertEqual(obj["geometry"]["type"],"curve")
                    self.assertIsNone(obj["name"])
                    self.assertEqual(obj["groups"],[])
                    self.assertTrue(obj["current_layer"])
                    self.assertFalse(obj["selected"])
        self.assertEqual(successes,92)

    def test_replay_compares_preserved_holes_and_uses_original_sources(self):
        fixture,observed=self.captures()
        native=copy.deepcopy(observed);native["engine"]="viboceros"
        client=Mock();client.run_viboceros.return_value=native
        self.assertTrue(replay(fixture,observed,client).passed)
        client.run_viboceros.assert_called_with(fixture,180)
        for field in ("weight","parameter","missing_hole"):
            bad=copy.deepcopy(observed)
            brep=bad["results"][4]["value"]["after"][-1]["geometry"]["definition"]
            if field=="weight":brep["edges"][0]["curve"]["definition"]["control_points"][0]["weight"]=2.
            elif field=="parameter":brep["faces"][0]["loops"][1][0]["definition"]["knots"][2]=.5
            else:brep["faces"][0]["loops"].pop()
            self.assertFalse(replay(fixture,bad,client).passed)
            self.assertEqual(client.run_viboceros.call_args.args[0],fixture)


if __name__=="__main__":unittest.main()
