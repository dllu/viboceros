"""Distribute macros and ownership, including every partial-failure boundary."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import test_worker


class DistributeWorkerTests(unittest.TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_macro_whitelists_options_and_preserves_signed_spacing_and_3d_points(self):
        self.worker.Rhino.Geometry = SimpleNamespace(Point3d=lambda x, y, z: SimpleNamespace(X=x, Y=y, Z=z))
        for mode in ["Gap", "Center"]:
            for direction in ["XAxis", "YAxis", "ZAxis"]:
                for spacing, token in [(None, "_Automatic"), (0, "0"), (-2.5, "-2.5"), (3, "3")]:
                    self.assertEqual(self.worker._distribute_script(dict(mode=mode, direction=direction, spacing=spacing)),
                                     f"_-Distribute _Mode=_{mode} _Spacing {token} _{direction}")
        operation = dict(mode="Center", direction="Direction", references=[[1, 2, 3], [3, 5, 10]])
        self.assertEqual(self.worker._distribute_script(operation),
                         "_-Distribute _Mode=_Center _Spacing _Automatic _Direction w1,2,3 w3,5,10")
        for changes in [dict(mode="Gap _Delete"), dict(direction="XAxis _Enter"),
                        dict(spacing=float("inf")), dict(references=[[0, 0, 0]]),
                        dict(references=[[0, 0, 0], [0, float("nan"), 1]])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._distribute_script(dict(operation, **changes))

    def test_invalid_preselection_and_groups_fail_before_document_access(self):
        operation = dict(mode="Gap", direction="XAxis", sources=[dict(type="point")] * 3)
        for changes in [dict(sources=[]), dict(sources=[{}] * 33), dict(selected=[]),
                        dict(selected=[0, 1]), dict(selected=[0, 0, 1]), dict(selected=[0, 1, 3]),
                        dict(groups=[[]]), dict(groups=[[0, 0]]), dict(groups=[[False, 1]]),
                        dict(groups=[[3]])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._distribute(dict(operation, **changes), {})

    def test_degenerate_direction_is_rejected_before_it_can_open_a_rhino_prompt(self):
        self.worker.Rhino.Geometry = SimpleNamespace(Point3d=lambda x, y, z: SimpleNamespace(X=x, Y=y, Z=z))
        for end in [[0, 0, 0], [1e-12, 0, 0]]:
            operation = dict(mode="Gap", direction="Direction", references=[[0, 0, 0], end],
                             sources=[dict(type="point")] * 3)
            with self.assertRaisesRegex(ValueError, "must be distinct"):
                self.worker._distribute(operation, dict(absolute=1e-9))

    def test_owned_objects_groups_planes_selection_and_disposables_survive_failures(self):
        for failure in [None, "initialization", "source", "group", "command", "record", "inspection", "insufficient", "stale"]:
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
                    SetConstructionPlane=lambda p, i=i: set_plane(i, p))) for i in range(2)]
                self.document.Views = SimpleNamespace(ActiveView=views[0], GetViewList=lambda a, b: views)
                aid = SimpleNamespace(UniversalConstructionPlaneMode=True)
                aid.GetCurrentState = lambda: aid.UniversalConstructionPlaneMode
                aid.UpdateFromState = lambda state: setattr(aid, "UniversalConstructionPlaneMode", state)
                self.worker.Rhino.ApplicationSettings = SimpleNamespace(ModelAidSettings=aid)
                selected = {"existing"}
                objects = {"existing": SimpleNamespace(Id="existing", IsSelected=lambda _: "existing" in selected)}
                owned, attributes, copies = [], [], []
                def duplicate():
                    local = SimpleNamespace(Dispose=Mock(), Transform=lambda _: failure != "inspection",
                                            GetBoundingBox=lambda _: SimpleNamespace(Min=[0, 0, 0], Max=[1, 2, 3]))
                    copies.append(local)
                    return local
                def source(_definition, _tolerance):
                    geometry = SimpleNamespace(Dispose=Mock(), Duplicate=duplicate)
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
                    objects[key] = SimpleNamespace(Id=key, Geometry=geometry, Attributes=attributes,
                                                   IsSelected=lambda _: key in selected)
                    return key
                self.document.Objects = SimpleNamespace(GetObjectList=lambda _: list(objects.values()),
                    UnselectAll=selected.clear, Select=selected.add, Delete=lambda key, _: objects.pop(key), AddCurve=add)
                self.document.Layers = SimpleNamespace(CurrentLayerIndex=7)
                entries = [None, ["existing"]]
                class Groups:
                    @property
                    def Count(self): return len(entries)
                    def IsDeleted(self, i): return entries[i] is None
                    def Delete(self, i): entries[i] = None
                    def GroupMembers(self, i): return [objects[key] for key in entries[i] if key in objects]
                    def Add(self, name, members):
                        entries[0] = members
                        if failure == "group": raise ValueError("group failure")
                        return 0
                self.document.Groups = Groups()
                self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=attrs)
                def make_plane(*args): return plane
                make_plane.WorldXY = object()
                self.worker.Rhino.Geometry = SimpleNamespace(Plane=make_plane, Transform=SimpleNamespace(PlaneToPlane=lambda a, b: object()))
                self.worker.Rhino.RhinoApp.RunScript = Mock()
                self.worker.Rhino.RhinoApp.CommandHistoryWindowText = "old: At least three groups of objects must be selected\n"
                self.worker.System = SimpleNamespace(Guid=SimpleNamespace(Empty="empty", NewGuid=lambda: "owned-group"))
                def run(script, verify):
                    self.assertEqual(script, "_-Distribute _Mode=_Gap _Spacing _Automatic _XAxis")
                    self.assertTrue(verify)
                    self.assertFalse(aid.UniversalConstructionPlaneMode)
                    self.assertEqual(selected, {"1", "2", "3"})
                    if failure == "insufficient":
                        self.worker.Rhino.RhinoApp.CommandHistoryWindowText += "At least three groups of objects must be selected\n"
                    if failure in ["command", "stale", "insufficient"]: raise ValueError("command failure")
                    return True
                operation = dict(mode="Gap", direction="XAxis", origin=[0, 0, 0], x_axis=[1, 0, 0],
                                 y_axis=[0, 1, 0], sources=[dict(type="line")] * 3, groups=[[0, 1]], selected=None, inspect=True)
                with patch.object(self.worker, "_point", side_effect=lambda p: p), \
                     patch.object(self.worker, "_vector", side_effect=lambda p: p), \
                     patch.object(self.worker, "_xyz", side_effect=lambda p: p), \
                     patch.object(self.worker, "_record_progress"), \
                     patch.object(self.worker, "_object_source", side_effect=source), \
                     patch.object(self.worker, "_run_surface_script", side_effect=run), \
                     patch.object(self.worker, "_plane_array_geometry_record", side_effect=ValueError("record failure") if failure == "record" else lambda *args: ([0, 1], [[0, 0, 0]])):
                    if failure not in [None, "insufficient"]:
                        with self.assertRaises(ValueError): self.worker._distribute(operation, {})
                    else:
                        value, elapsed = self.worker._distribute(operation, {})
                        self.assertEqual(value["succeeded"], failure != "insufficient")
                        self.assertEqual(value["groups"], [[0, 1]])
                        self.assertEqual(len(value["source_plane_bounds"]), 3)
                        self.assertTrue(all(r["retained"] and r["selected"] and r["current_layer"] for r in value["objects"]))
                        self.assertEqual(elapsed, 0)
                self.assertEqual(set(objects), {"existing"})
                self.assertEqual(selected, {"existing"})
                self.assertEqual(current_planes, original_planes)
                self.assertTrue(aid.UniversalConstructionPlaneMode)
                self.assertEqual(entries, [None, ["existing"]])
                self.assertEqual(self.worker.Rhino.RhinoApp.RunScript.call_count, 2)
                self.worker.Rhino.RhinoApp.RunScript.assert_called_with("!", False)
                for resource in owned + attributes + copies: resource.Dispose.assert_called_once_with()
