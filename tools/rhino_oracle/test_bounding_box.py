"""BoundingBox reports, whitelisted macros, and private-document ownership."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import test_worker


class BoundingBoxWorkerTests(unittest.TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_options_are_validated_before_accessing_or_mutating_the_document(self):
        operation = dict(coordinate_system="World", output="Solids", cumulative=True,
                         sources=[dict(type="point", point=[0,0,0])])
        for changes in [dict(coordinate_system="World _Delete"), dict(output="SubD"),
                        dict(cumulative="No"), dict(sources=[]), dict(sources=[{}]*17)]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._bounding_box_command(dict(operation, **changes), {})

    def test_cleanup_restores_planes_objects_groups_selection_and_disposes_owned_inputs(self):
        for failure in [None, "initialization", "source", "command", "record", "report"]:
            with self.subTest(failure=failure):
                original_planes = [object(), object()]
                current_planes = original_planes[:]
                plane = SimpleNamespace(IsValid=True)
                def set_plane(i, value):
                    current_planes[i] = value
                    if failure == "initialization" and value is plane:
                        raise ValueError("initialization failure")
                views = [SimpleNamespace(ActiveViewport=SimpleNamespace(
                    ConstructionPlane=lambda i=i: current_planes[i],
                    SetConstructionPlane=lambda p, i=i: set_plane(i,p))) for i in range(2)]
                self.document.Views = SimpleNamespace(ActiveView=views[0], GetViewList=lambda a,b: views)
                aid = SimpleNamespace(UniversalConstructionPlaneMode=True)
                aid.GetCurrentState = lambda: aid.UniversalConstructionPlaneMode
                aid.UpdateFromState = lambda state: setattr(aid,"UniversalConstructionPlaneMode",state)
                self.worker.Rhino.ApplicationSettings = SimpleNamespace(ModelAidSettings=aid)
                selected = {"existing"}
                objects = {"existing": SimpleNamespace(Id="existing", IsSelected=lambda _: "existing" in selected)}
                owned, attributes = [], []
                def source(_definition, _tolerance):
                    geometry = SimpleNamespace(Dispose=Mock())
                    owned.append(geometry)
                    return geometry
                def attrs():
                    a = SimpleNamespace(Dispose=Mock(), LayerIndex=0, Name=None)
                    attributes.append(a)
                    return a
                def add(geometry, attributes):
                    if failure == "source" and len(objects) == 2:
                        raise ValueError("source failure")
                    key = str(len(objects))
                    objects[key] = SimpleNamespace(Id=key,Geometry=geometry,Attributes=attributes,IsSelected=lambda _:key in selected)
                    return key
                self.document.Objects = SimpleNamespace(GetObjectList=lambda _:list(objects.values()),
                    UnselectAll=selected.clear, Select=selected.add, Delete=lambda key,_:objects.pop(key), AddCurve=add)
                self.document.Layers = SimpleNamespace(CurrentLayerIndex=0)
                entries = [None,["existing"]]
                class Groups:
                    @property
                    def Count(self): return len(entries)
                    def IsDeleted(self,i): return entries[i] is None
                    def Delete(self,i): entries[i] = None
                    def GroupMembers(self,i): return [objects[key] for key in entries[i] if key in objects]
                self.document.Groups = Groups()
                self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace,ObjectAttributes=attrs)
                self.worker.Rhino.Geometry = SimpleNamespace(Plane=lambda *args:plane)
                self.worker.Rhino.RhinoApp.RunScript = Mock()
                self.worker.Rhino.RhinoApp.CommandHistoryWindowText = "old history\n"
                self.worker.System = SimpleNamespace(Guid=SimpleNamespace(Empty="empty"))
                def run(script, verify):
                    self.assertEqual(script,"_-BoundingBox _CoordinateSystem=_CPlane _Cumulative=_No _Output=_Curves _Enter")
                    self.assertTrue(verify)
                    self.assertFalse(aid.UniversalConstructionPlaneMode)
                    entries[0] = [add(object(),SimpleNamespace(Name=None,LayerIndex=0))]
                    if failure == "command": raise ValueError("command failure")
                    if failure != "report":
                        self.worker.Rhino.RhinoApp.CommandHistoryWindowText += "CPlane coordinates:\ndimensions = 1,2,3\n"
                    return False  # Valid report-only commands can return False.
                operation = dict(coordinate_system="CPlane",cumulative=False,output="Curves",
                                 origin=[0,0,0],x_axis=[1,0,0],y_axis=[0,1,0],sources=[dict(type="line")]*2)
                with patch.object(self.worker,"_point",side_effect=lambda p:p), \
                     patch.object(self.worker,"_vector",side_effect=lambda p:p), \
                     patch.object(self.worker,"_record_progress"), \
                     patch.object(self.worker,"_object_source",side_effect=source), \
                     patch.object(self.worker,"_run_surface_script",side_effect=run), \
                     patch.object(self.worker,"_bounding_box_geometry_record",side_effect=ValueError("record failure") if failure=="record" else lambda g:dict(kind="curve",points=[[0,0,0]])):
                    if failure:
                        with self.assertRaises(ValueError):
                            self.worker._bounding_box_command(operation,{})
                    else:
                        value,elapsed = self.worker._bounding_box_command(operation,{})
                        self.assertTrue(value["succeeded"])
                        self.assertEqual(value["reported_boxes"],1)
                        self.assertEqual(value["group_sizes"],[1])
                        self.assertEqual(value["selected_sources"],[0,1])
                        self.assertEqual(value["sources_retained"],[0,1])
                        self.assertEqual(elapsed,0)
                self.assertEqual(set(objects),{"existing"})
                self.assertEqual(selected,{"existing"})
                self.assertEqual(current_planes,original_planes)
                self.assertTrue(aid.UniversalConstructionPlaneMode)
                self.assertEqual(entries,[None,["existing"]])
                for geometry in owned: geometry.Dispose.assert_called_once_with()
                for attribute in attributes: attribute.Dispose.assert_called_once_with()

    def test_source_builder_disposes_cloud_and_mesh_on_construction_failure(self):
        for kind in ["point_cloud","mesh"]:
            owned = SimpleNamespace(Dispose=Mock(),Add=Mock(side_effect=ValueError("add")),
                                    Vertices=SimpleNamespace(Add=Mock(side_effect=ValueError("add"))))
            self.worker.Rhino.Geometry = SimpleNamespace(PointCloud=lambda:owned,Mesh=lambda:owned)
            definition = dict(type=kind,points=[[0,0,0]],vertices=[[0,0,0]],faces=[])
            with patch.object(self.worker,"_point",side_effect=lambda p:p), self.assertRaisesRegex(ValueError,"add"):
                self.worker._object_source(definition,{})
            owned.Dispose.assert_called_once_with()
