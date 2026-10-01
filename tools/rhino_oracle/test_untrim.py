import copy
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import join_probe
from .client import OracleProtocolError
from .untrim_cases import request
from .untrim_probe import run, validate
from .untrim_replay import replay


class UntrimTests(unittest.TestCase):
    def test_replay_uses_only_sources_and_detects_changed_geometry_and_attributes(self):
        root=Path(__file__).parent
        fixture=json.loads((root/"fixtures/untrim_all.json").read_text())
        observed=json.loads((root/"observations/untrim_all.json").read_text())
        native=copy.deepcopy(observed);native["engine"]="viboceros"
        client=Mock();client.run_viboceros.return_value=native
        self.assertTrue(replay(fixture,observed,client).passed)
        client.run_viboceros.assert_called_with(fixture,180)
        for field in ("curve","color","orientation"):
            bad=copy.deepcopy(observed)
            after=bad["results"][2]["value"]["after"]
            if field=="curve":after[0]["geometry"]["definition"]["control_points"][0]["weight"]=2.
            elif field=="color":after[0]["color"][0]=99
            else:after[-1]["geometry"]["definition"]["topology"]["faces"][0]["reversed"]=True
            self.assertFalse(replay(fixture,bad,client).passed)
            self.assertEqual(client.run_viboceros.call_args.args[0],fixture)
        box=copy.deepcopy(observed)
        box["results"][12]["value"]["after"][0]["geometry"]["definition"]["vertices"][0]["point"][0]+=1.
        with self.assertRaisesRegex(OracleProtocolError,"changed box geometry"):
            replay(fixture,box,client)

    def test_source_only_fixtures_and_complete_native_definitions(self):
        root=Path(__file__).parent
        fixture=json.loads((root/"fixtures/untrim_all.json").read_text())
        observed=json.loads((root/"observations/untrim_all.json").read_text())
        self.assertEqual(request(),fixture)
        self.assertEqual(len(fixture["operations"]),72)
        self.assertEqual(len(observed["results"]),72)
        def curve_definition(spec,actual):
            self.assertEqual(spec["degree"],actual["degree"])
            self.assertEqual(spec["knots"],actual["knots"])
            expected=copy.deepcopy(spec["control_points"])
            if len(actual["control_points"][0]["point"])==2:
                for control in expected:control["point"]=control["point"][:2]
            self.assertEqual(expected,actual["control_points"])
        def surface_definition(spec,actual):
            self.assertEqual([spec["degree_u"],spec["degree_v"]],actual["degree"])
            self.assertEqual([spec["control_point_count_u"],spec["control_point_count_v"]],actual["control_count"])
            for key in ("knots_u","knots_v","control_points"): self.assertEqual(spec[key],actual[key])
        for op,row in zip(fixture["operations"],observed["results"]):
            validate(op)
            self.assertEqual(op["id"],row["id"])
            value=row["value"]
            self.assertEqual(len(value["constructed"]),len(op["sources"]))
            for spec,created in zip(op["sources"],value["constructed"]):
                if spec["type"]=="surface":surface_definition(spec,created["definition"])
                elif spec["type"]=="brep":
                    brep=created["definition"]
                    for i,face in enumerate(brep["faces"]):
                        surface_definition(spec["surface"] if i==0 else spec["cap_surface"],face["definition"])
                        self.assertEqual(len(spec["boundaries"]),len(face["loops"]))
                        for boundary,loop in zip(spec["boundaries"],face["loops"]):
                            self.assertEqual(len(loop),1)
                            curve_definition(boundary["parameter_curve"],loop[0]["definition"])
                    self.assertEqual(len(spec["boundaries"]),len(brep["edges"]))
                    for boundary,edge in zip(spec["boundaries"],brep["edges"]):
                        curve_definition(boundary["curve"],edge["curve"]["definition"])
                elif spec["type"]=="point":self.assertEqual(spec["point"],created["point"])
            if not value["succeeded"]:
                self.assertIn(op["id"].split("-keep-")[0],("box","paraboloid-oblique"))
                for before,after in zip(value["before"],value["after"]):
                    self.assertEqual(before["geometry"],after["geometry"])
                continue
            sources={obj["source"]:obj for obj in value["after"] if obj["source"] is not None}
            for before in value["before"]:
                after=sources[before["source"]]
                for key in ("name","groups","current_layer","color","color_source"):self.assertEqual(before[key],after[key])
                self.assertEqual(after["selected"],op["preselect"])
                if after["geometry"]["type"]=="brep":
                    self.assertEqual(after["geometry"]["untrimmed"],[True])
                    self.assertEqual(before["geometry"]["definition"]["faces"][0]["definition"],
                                     after["geometry"]["definition"]["faces"][0]["definition"])
            for obj in value["after"]:
                if obj["source"] is None:
                    self.assertTrue(op["keep_trim_objects"])
                    self.assertEqual(obj["geometry"]["type"],"curve")
                    self.assertIsNone(obj["name"])
                    self.assertEqual(obj["groups"],[])
                    self.assertTrue(obj["current_layer"])
                    self.assertFalse(obj["selected"])

    def test_invalid_operations_are_rejected_before_geometry_construction(self):
        base=request()["operations"][0]
        for changes in (dict(id="x _Delete"),dict(keep_trim_objects=1),dict(preselect=None),
                        dict(sources=[]),dict(sources=[{}]*9),dict(sources=[None]),
                        dict(sources=[dict(type="macro")]),dict(extra=1)):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(base,**changes))

    def test_failed_insertion_and_cleanup_still_restore_selection_and_dispose_sources(self):
        class Brep:
            Faces=[SimpleNamespace(IsSurface=False)]
            def __init__(self):self.Dispose=Mock()
        for cleanup_fails in (False,True):
            with self.subTest(cleanup_fails=cleanup_fails):
                g=Brep()
                Brep.CreateFromBox=Mock(return_value=g)
                attrs=SimpleNamespace(Dispose=Mock())
                original=SimpleNamespace(Id="original",IsSelected=lambda _:True)
                table=SimpleNamespace(GetObjectList=lambda _:[original],AddBrep=Mock(return_value="empty"),
                    UnselectAll=Mock(side_effect=[None,RuntimeError("unselect") if cleanup_fails else None]),
                    Select=Mock(),Delete=Mock())
                scripts=Mock(side_effect=RuntimeError("cancel") if cleanup_fails else None)
                rhino=SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep,Surface=type("Surface",(),{}),
                        Curve=type("Curve",(),{}),Point=type("Point",(),{}),BoundingBox=Mock(),Point3d=Mock()),
                    DocObjects=SimpleNamespace(ObjectEnumeratorSettings=lambda:SimpleNamespace(),ObjectAttributes=lambda:attrs,
                        ObjectColorSource=SimpleNamespace(ColorFromObject="ColorFromObject")),
                    RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(Objects=table)),RhinoApp=SimpleNamespace(RunScript=scripts))
                host=dict(Rhino=rhino,System=SimpleNamespace(Guid=SimpleNamespace(Empty="empty"),
                    Drawing=SimpleNamespace(Color=SimpleNamespace(FromArgb=Mock()))),
                          _interchange_brep_record=Mock(return_value={"full":"definition"}))
                op=dict(op="untrim_all_command",id="failure",sources=[dict(type="box")],keep_trim_objects=False)
                with patch.dict("sys.modules",dict(join_probe=join_probe)),self.assertRaises(ValueError) as error:
                    run(op,{},host)
                self.assertIn("cleanup failed" if cleanup_fails else "insertion failed",str(error.exception))
                table.Select.assert_called_once_with("original")
                table.Delete.assert_not_called()
                g.Dispose.assert_called_once_with()
                attrs.Dispose.assert_called_once_with()


if __name__=="__main__":unittest.main()
