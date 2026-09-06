"""Array command macros, cleanup ownership, and signed bounds-fixture weights."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import test_worker


class PlaneArrayWorkerTests(unittest.TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_array_macros_whitelist_options_and_force_requested_fill_preview_lengths(self):
        operation = dict(command="Array", counts=[3, 1, 2], distances=[4, 0, -6], mode="Fill")
        self.assertEqual(self.worker._plane_array_script(operation), "_-Array _Mode=_Fill 3 1 2 4 -6 _Enter")
        self.assertEqual(self.worker._plane_array_script(dict(operation, explicit_lengths=True)),
                         "_-Array _Mode=_Fill 3 1 2 4 -6 _XLength 4 _ZLength -6 _Enter")
        for changes in [dict(counts=[1, 1, 1]), dict(counts=[True, 2, 1]), dict(counts=[100, 2, 1]),
                        dict(counts=[20, 20, 1]), dict(mode="Fill _Delete"), dict(distances=[1, float("inf"), 2])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._plane_array_script(dict(operation, **changes))
        polar = dict(command="ArrayPolar", item_count=4, center=[1, 2, 3], angle=-360, rotate=False, z_offset=2)
        with patch.object(self.worker, "_command_point", return_value="1,2,3"):
            self.assertEqual(self.worker._plane_array_script(polar), "_-ArrayPolar w1,2,3 4 _Rotate=_No _ZOffset 2 -360 _Enter")
            self.assertEqual(self.worker._plane_array_script(dict(command="ArrayLinear", item_count=3, references=[[1, 2, 3]] * 2)),
                             "_ArrayLinear 3 w1,2,3 w1,2,3")
            for changes in [dict(command="Delete"), dict(angle=0), dict(item_count=1), dict(rotate="Yes"), dict(z_offset=float("nan"))]:
                with self.subTest(changes=changes), self.assertRaises(ValueError):
                    self.worker._plane_array_script(dict(polar, **changes))

    def test_signed_curve_weights_are_forwarded_but_zero_and_nonfinite_weights_are_rejected(self):
        table = SimpleNamespace(Count=1, SetPoint=Mock(return_value=True))
        with patch.object(self.worker, "_point", side_effect=lambda p: p):
            for weight in [-1, -0.25, 1e-200, 1e200]:
                self.worker._set_curve_controls(SimpleNamespace(Points=table), [{"point": [1, 2, 3], "weight": weight}])
                table.SetPoint.assert_called_with(0, [1, 2, 3], weight)
            for weight in [0, float("nan"), float("inf")]:
                with self.assertRaises(ValueError):
                    self.worker._set_curve_controls(SimpleNamespace(Points=table), [{"point": [1, 2, 3], "weight": weight}])

    def test_array_cleanup_preserves_existing_groups_even_when_deleted_slots_are_reused(self):
        for failure in [None, "initialization", "source", "group", "command", "record"]:
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
                curves = []
                def curve_input(_):
                    curve = SimpleNamespace(Dispose=Mock(), Domain=SimpleNamespace(T0=0, T1=1, ParameterAt=lambda t: t),
                                            PointAt=lambda t: [t, 2, 3])
                    curves.append(curve)
                    return curve
                def add(curve, attributes):
                    if failure == "source" and len(objects) == 2:
                        raise ValueError("source failure")
                    key = str(len(objects))
                    objects[key] = SimpleNamespace(Id=key, Geometry=curve, Attributes=attributes, IsSelected=lambda _: key in selected)
                    return key
                self.document.Objects = SimpleNamespace(GetObjectList=lambda _: list(objects.values()), UnselectAll=selected.clear,
                    Select=selected.add, Delete=lambda key, _: objects.pop(key), AddCurve=add)
                entries = [None, ["existing"]]
                class Groups:
                    @property
                    def Count(self): return len(entries)
                    def IsDeleted(self, i): return entries[i] is None
                    def GroupMembers(self, i): return [objects[key] for key in entries[i] if key in objects]
                    def Delete(self, i): entries[i] = None
                    def Add(self, name, members):
                        index = entries.index(None) if None in entries else len(entries)
                        if index == len(entries): entries.append(list(members))
                        else: entries[index] = list(members)
                        if failure == "group": raise ValueError("group failure")
                        return index
                self.document.Groups = Groups()
                self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=SimpleNamespace)
                self.worker.Rhino.Geometry = SimpleNamespace(Plane=lambda *args: plane)
                self.worker.Rhino.RhinoApp.RunScript = Mock()
                self.worker.Rhino.RhinoApp.CommandHistoryWindowText = ""
                self.worker.System = SimpleNamespace(Guid=SimpleNamespace(Empty="empty", NewGuid=lambda: "owned"))
                def run(script, verify):
                    self.assertTrue(verify)
                    self.assertFalse(aid.UniversalConstructionPlaneMode)
                    ids = [add(obj.Geometry, obj.Attributes) for obj in list(objects.values()) if obj.Id != "existing"]
                    self.document.Groups.Add("copies", ids)
                    if failure == "command": raise ValueError("command failure")
                    return True
                operation = dict(command="Array", counts=[2, 1, 1], distances=[4, 0, 0], mode="UnitCell",
                                 origin=[0, 0, 0], x_axis=[1, 0, 0], y_axis=[0, 1, 0], sources=[{}, {}])
                with patch.object(self.worker, "_point", side_effect=lambda p: p), patch.object(self.worker, "_vector", side_effect=lambda p: p), \
                     patch.object(self.worker, "_join_close_input", side_effect=curve_input), patch.object(self.worker, "_record_progress"), \
                     patch.object(self.worker, "_run_surface_script", side_effect=run), \
                     patch.object(self.worker, "_xyz", side_effect=ValueError("record failure") if failure == "record" else lambda p: p):
                    if failure:
                        with self.assertRaisesRegex(ValueError, failure + " failure"):
                            self.worker._plane_array(operation)
                    else:
                        value, elapsed = self.worker._plane_array(operation)
                        self.assertEqual(len(value["objects"]), 4)
                        self.assertEqual(value["groups"], [[0, 1], [0, 1]])
                        self.assertEqual(elapsed, 0)
                self.assertEqual(set(objects), {"existing"})
                self.assertEqual(selected, {"existing"})
                self.assertEqual(current_planes, original_planes)
                self.assertTrue(aid.UniversalConstructionPlaneMode)
                self.assertEqual(entries[1], ["existing"])
                self.assertTrue(all(entry is None for i, entry in enumerate(entries) if i != 1))
                for curve in curves: curve.Dispose.assert_called_once_with()
