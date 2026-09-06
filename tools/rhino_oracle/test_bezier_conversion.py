"""Bezier command probes: exact metadata, preflight, and resource ownership."""
from contextlib import nullcontext
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from . import test_worker


class BezierWorkerTests(unittest.TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_invalid_requests_never_access_the_document(self):
        valid = dict(sources=[dict(type="nurbs")], delete_input=True)
        for changes in [dict(sources=[]), dict(sources=[{}]*17), dict(delete_input=1),
                        dict(selected=[]), dict(selected=[True]), dict(selected=[-1]),
                        dict(selected=[1]), dict(selected=[0,0]), dict(sources=[dict(type="point")]),
                        dict(sources=[dict(type="brep",cap_surface={})])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.worker._geometry_conversion(dict(valid,**changes), {})

    def test_cleanup_and_preselection_at_every_failure_boundary(self):
        for failure in [None,"plane","layer","current","source","insert","group","select","incomplete","record","command"]:
            with self.subTest(failure=failure): self.exercise(failure)

    def test_command_macros_cover_explicit_and_default_choices(self):
        for delete in [False,True,None]:
            with self.subTest(delete=delete): self.exercise(None,delete)

    def test_geometrically_identical_outputs_use_creation_order_for_canonical_ties(self):
        self.exercise(None,True,copies=2,reverse_enumerator=True)

    def test_undo_after_conversion_restores_only_owned_objects_before_cleanup(self):
        for delete in [False,True]:
            self.exercise(None,delete,undo_after=True)
        self.exercise("noop-undo",False,undo_after=True)

    def test_nurbs_replacement_and_copy_cleanup_at_every_failure_boundary(self):
        for delete in [False,True]:
            for failure in [None,"plane","layer","current","source","insert","group","select","incomplete","record","command"]:
                with self.subTest(delete=delete,failure=failure): self.exercise(failure,delete,conversion="ToNURBS")
            self.exercise(None,delete,undo_after=True,conversion="ToNURBS")
        self.exercise("noop-undo",False,undo_after=True,conversion="ToNURBS")

    def exercise(self,failure,delete=True,copies=1,reverse_enumerator=False,undo_after=False,conversion="ConvertToBeziers"):
        worker,doc=self.worker,self.document
        selected={"existing"}; objects={}; owned=[]; attributes=[]; layers=[]
        class Curve:
            def __init__(self,representation="LineCurve"): self.Dispose=Mock();self.representation=representation
            def GetType(self): return SimpleNamespace(Name=self.representation)
        class NotCurve: pass
        class Attributes:
            def __init__(self):
                self.Name=None; self.LayerIndex=0; self.members=[]; self.Dispose=Mock()
                self.ObjectColor=SimpleNamespace(R=0,G=0,B=0); self.ColorSource="ColorFromLayer"
                attributes.append(self)
            def GetGroupList(self): return self.members[:]
        class Layer:
            def __init__(self): self.Name=""; self.Dispose=Mock(); layers.append(self)
        worker.Rhino.Geometry=SimpleNamespace(Curve=Curve,Point=NotCurve,Mesh=NotCurve,PointCloud=NotCurve,Brep=NotCurve,Surface=NotCurve,Plane=SimpleNamespace(WorldXY="world"))
        worker.Rhino.DocObjects=SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace,Layer=Layer,ObjectAttributes=Attributes,ObjectColorSource=SimpleNamespace(ColorFromObject="ColorFromObject"))
        worker.System.Guid=SimpleNamespace(NewGuid=lambda:"unique",Empty="empty")
        worker.System.Drawing=SimpleNamespace(Color=SimpleNamespace(FromArgb=lambda r,g,b:SimpleNamespace(R=r,G=g,B=b)))
        worker.Rhino.RhinoApp.RunScript=Mock()
        def obj(key,geometry,attrs,serial):
            return SimpleNamespace(Id=key,Geometry=geometry,Attributes=attrs,RuntimeSerialNumber=serial,IsSelected=lambda _:key in selected)
        objects["existing"]=obj("existing",None,None,0)
        def insert(geometry,attrs):
            if failure=="insert": return "empty"
            objects["source"]=obj("source",geometry,attrs,1)
            return "source"
        def select(key):
            if key=="source" and failure=="select": return False
            if not (key=="source" and failure=="incomplete"): selected.add(key)
            return True
        def remove(key,_):
            self.assertNotEqual(key,"existing"); objects.pop(key); selected.discard(key)
        doc.Objects=SimpleNamespace(GetObjectList=lambda _:list(objects.values())[::(-1 if reverse_enumerator else 1)],UnselectAll=selected.clear,Select=select,FindId=objects.get,AddCurve=insert,Delete=remove)
        layer_entries={0:"existing"}
        def add_layer(layer):
            if failure=="layer" and len(layer_entries)==2: return -1
            i=len(layer_entries); layer_entries[i]=layer.Name; return i
        def current(i,_):
            if failure=="current" and i!=0: return False
            doc.Layers.CurrentLayerIndex=i; return True
        def delete_layer(i,_):
            self.assertNotEqual(i,0); del layer_entries[i]
        doc.Layers=SimpleNamespace(CurrentLayerIndex=0,Add=add_layer,SetCurrentLayerIndex=current,Delete=delete_layer)
        groups={0:("existing",[])}; deleted=set()
        def add_group(name,members):
            if failure=="group" and len(groups)==2: return -1
            i=len(groups); groups[i]=(name,list(members)); doc.Groups.Count=len(groups)
            for key in members: objects[key].Attributes.members.append(i)
            return i
        def delete_group(i):
            self.assertNotEqual(i,0); deleted.add(i)
        doc.Groups=SimpleNamespace(Count=1,IsDeleted=lambda i:i in deleted,GroupName=lambda i:groups[i][0],Add=add_group,Delete=delete_group,
                                   GroupMembers=lambda i:[objects[k] for k in groups[i][1] if k in objects])
        def source(*_):
            if failure=="source": raise ValueError("source failure")
            c=Curve(); owned.append(c); return c
        def plane(_):
            if failure=="plane": raise ValueError("plane failure")
        viewport=SimpleNamespace(SetConstructionPlane=plane)
        def sample(*_):
            if failure=="record": raise ValueError("record failure")
            return [0,1],[[0,0,0],[1,1,0]]
        saved_source=[]
        def command(script,verify):
            if script=="_Undo":
                self.assertTrue(undo_after)
                for key in list(objects):
                    if key.startswith("output-"): remove(key,True)
                if delete is not False: objects["source"]=saved_source[0]
                return True
            expected="_ToNURBS"+(" _DeleteInputObjects="+("Yes" if delete else "No") if delete is not None else "")+" _Enter" if conversion=="ToNURBS" else "_ConvertToBeziers "+("_Enter" if delete is None else "_Yes" if delete else "_No")
            self.assertEqual(script,expected)
            self.assertTrue(verify)
            if failure=="noop-undo": return True
            saved_source.append(objects["source"])
            if conversion=="ToNURBS" and delete is not False:
                objects["source"]=obj("source",Curve("NurbsCurve"),objects["source"].Attributes,2)
                if failure=="command": raise ValueError("replacement failure")
                return True
            # Even a partially failed command may have added an object.
            for i in range(copies):
                key="output-%d" % i
                objects[key]=obj(key,Curve("NurbsCurve"),objects["source"].Attributes if conversion=="ToNURBS" else Attributes(),2+i)
                if conversion!="ToNURBS": objects[key].Attributes.LayerIndex=2
            if delete is not False: remove("source",True)
            if failure=="command": raise ValueError("command failure")
        with patch.object(worker,"_independent_construction_planes",return_value=nullcontext(viewport)), \
             patch.object(worker,"_object_source",side_effect=source), \
             patch.object(worker,"_plane_array_geometry_record",side_effect=sample), \
             patch.object(worker,"_nurbs_curve_definition",return_value={"degree":1}), \
             patch.object(worker,"_run_surface_script",side_effect=command) as run, \
             patch.object(worker,"_record_progress"):
            operation=dict(sources=[dict(type="line" if conversion=="ToNURBS" else "nurbs")],delete_input=delete,undo_after=undo_after)
            if failure:
                with self.assertRaises(ValueError): worker._geometry_conversion(operation,{},conversion)
            else:
                value,_=worker._geometry_conversion(operation,{},conversion)
                self.assertEqual(value["before"]["objects"][0]["groups"],["Group-0","Group-1"])
                output=value["after"]["objects"][-1]
                expected=(0 if delete is not False else None,"Source","source-0",delete is not False,["Group-0","Group-1"]) if conversion=="ToNURBS" else (None,"Current",None,False,[])
                self.assertEqual((output["original"],output["layer"],output["name"],output["selected"],output["groups"]),expected)
                self.assertEqual(len(value["after"]["groups"]),3)
                self.assertEqual(value["after"]["creation_order"],list(range(len(value["after"]["objects"]))))
            if failure not in [None,"command","noop-undo"]: run.assert_not_called()
            if failure=="noop-undo": self.assertEqual(run.call_count,1)
        self.assertEqual(set(objects),{"existing"}); self.assertEqual(selected,{"existing"})
        self.assertEqual(layer_entries,{0:"existing"}); self.assertEqual(doc.Layers.CurrentLayerIndex,0)
        self.assertEqual(set(groups)-deleted,{0})
        for c in owned: c.Dispose.assert_called_once()
        for a in attributes:
            if a.Name: a.Dispose.assert_called_once()
        for layer in layers: layer.Dispose.assert_called_once()
