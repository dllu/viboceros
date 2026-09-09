"""Ordered group probes: preflight, exact records, and private resource ownership."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import test_worker


class GroupMembershipWorkerTests(unittest.TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_invalid_indices_commands_and_deleted_groups_fail_before_document_access(self):
        operation = dict(sources=[dict(type="point")], groups=[[0]], steps=[])
        invalid = [dict(sources=[]), dict(sources=[{}] * 33), dict(groups=[[]] * 17),
                   dict(steps=[dict(kind="recall_previous", deselect_others="Yes")]),
                   dict(groups=[[0, 0]]), dict(groups=[[True]]), dict(groups=[[1]]),
                   dict(steps=[dict(kind="select", objects=[0, 0])]),
                   dict(steps=[dict(kind="set", object=0, groups=[0, 0])]),
                   dict(steps=[dict(kind="add", group=1, objects=[0])]),
                   dict(steps=[dict(kind="add_to_group", group=True)]),
                   dict(steps=[dict(kind="add_to_group", group=1)]),
                   dict(steps=[dict(kind="delete_group", group=0), dict(kind="add_to_group", group=0)]),
                   dict(steps=[dict(kind="command", name="Copy _Delete")]),
                   dict(steps=[dict(kind="select", objects=[])] * 65)]
        for step in [dict(kind="set", object=0, groups=[0]), dict(kind="add", group=0, objects=[0]),
                     dict(kind="delete_group", group=0)]:
            invalid.append(dict(steps=[dict(kind="delete_group", group=0), step]))
        for changes in invalid:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._group_memberships(dict(operation, **changes), {})

    def test_records_order_noop_add_auto_names_and_cleanup_at_each_failure_boundary(self):
        for failure in [None, "initialization", "source", "group", "duplicate", "attributes", "command", "record"]:
            with self.subTest(failure=failure):
                self.exercise(failure)

    def test_existing_group_names_are_rejected_without_mutating_the_document(self):
        self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace)
        self.document.Objects = SimpleNamespace(GetObjectList=lambda _: [], UnselectAll=Mock())
        self.document.Groups = SimpleNamespace(Count=1, IsDeleted=lambda _: False, GroupName=lambda _: "Group-0", Add=Mock())
        with self.assertRaisesRegex(ValueError, "collides"):
            self.worker._group_memberships(dict(sources=[dict(type="point")],groups=[[0]],steps=[]), {})
        self.document.Objects.UnselectAll.assert_not_called()
        self.document.Groups.Add.assert_not_called()

    def test_missing_sources_and_incomplete_preselection_never_run_commands(self):
        for failure in ["missing-set", "missing-add", "missing-select", "unselected", "ungroup-empty", "distribute-two", "add-to-group-unselected"]:
            with self.subTest(failure=failure):
                self.exercise(failure)

    def test_command_macros_complete_the_observed_prompts(self):
        for command in ["Copy", "Array", "ArrayLinear", "ArrayPolar", "Explode", "ConvertToBeziers", "Ungroup", "UngroupAll", "AddToGroup"]:
            with self.subTest(command=command): self.exercise(None, command)

    def test_nameless_output_provenance_is_only_inferred_for_a_single_source(self):
        self.exercise(None, "ConvertToBeziers", nameless=True)
        self.exercise("ambiguous", nameless=True)

    def exercise(self, failure, command="Copy", nameless=False):
        original_planes = [object(), object()]
        current_planes = original_planes[:]
        plane = object()
        def set_plane(i, value):
            current_planes[i] = value
            if failure == "initialization" and value is plane: raise ValueError("initialization failure")
        views = [SimpleNamespace(ActiveViewport=SimpleNamespace(
            ConstructionPlane=lambda i=i: current_planes[i],
            SetConstructionPlane=lambda p, i=i: set_plane(i, p))) for i in range(2)]
        self.document.Views = SimpleNamespace(ActiveView=views[0], GetViewList=lambda a, b: views)
        aid = SimpleNamespace(UniversalConstructionPlaneMode=True)
        aid.GetCurrentState = lambda: aid.UniversalConstructionPlaneMode
        aid.UpdateFromState = lambda state: setattr(aid, "UniversalConstructionPlaneMode", state)
        self.worker.Rhino.ApplicationSettings = SimpleNamespace(ModelAidSettings=aid)
        selected, owned, disposable_attributes = {"existing"}, [], []
        class Attributes:
            def __init__(self):
                self.Name, self.LayerIndex, self.memberships = None, 0, []
                self.Dispose = Mock()
                disposable_attributes.append(self)
            def GetGroupList(self): return self.memberships[:]
            def RemoveFromAllGroups(self): self.memberships.clear()
            def AddToGroup(self, i):
                if i not in self.memberships: self.memberships.append(i)
            def Duplicate(self):
                if failure == "duplicate": raise ValueError("duplicate failure")
                other = Attributes()
                other.Name, other.LayerIndex, other.memberships = self.Name, self.LayerIndex, self.memberships[:]
                return other
        objects = {"existing": SimpleNamespace(Id="existing", IsSelected=lambda _: "existing" in selected)}
        def source(_definition, _tolerance):
            geometry = SimpleNamespace(Dispose=Mock())
            owned.append(geometry)
            return geometry
        def add(geometry, attributes):
            key = "object-%d" % len(objects)
            objects[key] = SimpleNamespace(Id=key, Geometry=geometry, Attributes=attributes,
                                           IsSelected=lambda _: key in selected)
            if failure == "source": raise ValueError("source failure")
            return key
        def modify(key, attributes, _quiet):
            if failure == "attributes": return False
            objects[key].Attributes = attributes
            return True
        def find(key):
            if failure in ("missing-set", "missing-add", "missing-select"): return None
            return objects.get(key)
        self.document.Objects = SimpleNamespace(GetObjectList=lambda _: list(objects.values()),
            UnselectAll=selected.clear, Select=selected.add, Delete=lambda key, _: objects.pop(key),
            AddCurve=add, ModifyAttributes=modify, FindId=find)
        self.document.Layers = SimpleNamespace(CurrentLayerIndex=7)
        entries = [None, "existing-group"]
        class Groups:
            @property
            def Count(self): return len(entries)
            def IsDeleted(self, i): return entries[i] is None
            def GroupName(self, i): return entries[i]
            def Delete(self, i):
                entries[i] = None
                for key, obj in objects.items():
                    if key != "existing" and i in obj.Attributes.memberships: obj.Attributes.memberships.remove(i)
                return True
            def GroupMembers(self, i):
                return [obj for key, obj in objects.items() if key != "existing" and i in obj.Attributes.memberships]
            def Add(self, name, members):
                if None in entries:
                    index = entries.index(None)
                    entries[index] = name
                else:
                    index = len(entries)
                    entries.append(name)
                for key in members: objects[key].Attributes.AddToGroup(index)
                if failure == "group": raise ValueError("group failure")
                return index
            def AddToGroup(self, index, members):
                for key in members: objects[key].Attributes.AddToGroup(index)
                return False  # Rhino returns false for an all-existing no-op.
        groups = Groups()
        self.document.Groups = groups
        self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=Attributes)
        self.worker.Rhino.Geometry = SimpleNamespace(Plane=SimpleNamespace(WorldXY=plane),
            **{name: type(name, (), {}) for name in ["Point", "PointCloud", "Mesh", "Brep", "Surface"]})
        self.worker.Rhino.RhinoApp.RunScript = Mock()
        self.worker.System = SimpleNamespace(Guid=SimpleNamespace(Empty="empty"))
        def run(script, verify):
            self.assertEqual(script, {"Copy":"_Copy w0,0,0 w10,0,0 _Enter",
                "AddToGroup":"_AddToGroup Group-0 _Enter",
                "Array":"_-Array _Mode=_UnitCell 2 1 1 10 _Enter",
                "ArrayPolar":"_-ArrayPolar w0,0,0 2 _Rotate=_Yes _ZOffset 0 180 _Enter",
                "ArrayLinear":"_ArrayLinear 2 w0,0,0 w10,0,0", "Explode":"_Explode",
                "ConvertToBeziers":"_ConvertToBeziers _Yes", "Ungroup":"_Ungroup", "UngroupAll":"_UngroupAll"}[command])
            self.assertTrue(verify)
            self.assertFalse(aid.UniversalConstructionPlaneMode)
            self.assertEqual(selected, {"object-1"} if nameless and failure is None else {"object-1", "object-2"})
            # Exercise a new command-created definition reusing a deleted slot.
            groups.Delete(0)
            groups.Add("Group107", ["object-1"])
            groups.Add("Group108", ["object-2"])
            groups.Add("Group-0 copy", [])  # Wrong style must NOT be canonicalized.
            if nameless:
                # This mimics attributes created and owned by Rhino's command,
                # not resources allocated by the Python worker.
                attributes = SimpleNamespace(Name=None, GetGroupList=lambda: [], memberships=[])
                add(owned[0], attributes)
            if failure == "command": raise ValueError("command failure")
        set_step = dict(kind="set", object=0, groups=[1, 0])
        add_step = dict(kind="add", group=0, objects=[0, 0])
        select_step = dict(kind="select", objects=[0] if nameless and failure is None else [0, 1])
        steps = [set_step, add_step, select_step, dict(kind="command", name=command)]
        if command == "AddToGroup": steps[-1] = dict(kind="add_to_group", group=0)
        if failure == "add-to-group-unselected": steps = [dict(kind="add_to_group", group=0)]
        if failure == "missing-add": steps = [add_step]
        if failure == "missing-select": steps = [select_step]
        if failure == "unselected": steps = [dict(kind="command", name="Copy")]
        if failure == "ungroup-empty":
            steps = [dict(kind="set", object=i, groups=[]) for i in range(2)] + [select_step, dict(kind="command", name="Ungroup")]
        if failure == "distribute-two": steps[-1] = dict(kind="command", name="Distribute")
        operation = dict(sources=[dict(type="line")] * 2, groups=[[0, 1], [0]], steps=steps)
        with patch.object(self.worker, "_record_progress"), \
             patch.object(self.worker, "_object_source", side_effect=source), \
             patch.object(self.worker, "_run_surface_script", side_effect=run) as run_mock, \
             patch.object(self.worker, "_plane_array_geometry_record", side_effect=ValueError("record failure") if failure == "record" else lambda *args: ([0, 1], [[0, 0, 0]])):
            if failure:
                with self.assertRaises(ValueError): self.worker._group_memberships(operation, {})
            else:
                value, elapsed = self.worker._group_memberships(operation, {})
                states = value["states"]
                self.assertEqual(states[1]["objects"][0]["groups"], ["Group-1", "Group-0"])
                self.assertEqual(states[1], states[2])
                original_zero = next(o for o in states[-1]["objects"] if o["source"]==0 and o["retained"])
                self.assertEqual(original_zero["groups"], ["Group-1", "CopyGroup-0"])
                source_one = next(o for o in states[-1]["objects"] if o["source"]==1)
                self.assertEqual(source_one["groups"], ["CopyGroup-1"])
                if nameless:
                    output = next(o for o in states[-1]["objects"] if o["name"] is None)
                    self.assertEqual(output["source"], 0)
                    self.assertFalse(output["retained"])
                self.assertIn("Group-0 copy", [g["name"] for g in states[-1]["groups"]])
                self.assertEqual(elapsed, 0)
            self.assertEqual(run_mock.call_count, int(failure in (None, "command", "ambiguous")))
        self.assertEqual(set(objects), {"existing"})
        self.assertEqual(selected, {"existing"})
        self.assertEqual(current_planes, original_planes)
        self.assertTrue(aid.UniversalConstructionPlaneMode)
        self.assertEqual([(i, name) for i, name in enumerate(entries) if name is not None], [(1, "existing-group")])
        self.assertEqual(self.worker.Rhino.RhinoApp.RunScript.call_count, 2)
        for resource in owned + disposable_attributes: resource.Dispose.assert_called_once_with()
