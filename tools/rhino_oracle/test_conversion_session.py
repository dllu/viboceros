"""Self-seeded option probes cannot depend on unrelated Rhino session history."""
from unittest.mock import Mock,patch
from types import SimpleNamespace
import unittest
from . import test_worker


class ConversionSessionTests(unittest.TestCase):
    def setUp(self): test_worker.RhinoWorkerTests.setUp(self)

    def test_mesh_conversion_option_seed_owns_only_its_inserted_objects(self):
        for failure in [None,"insert","command"]:
            with self.subTest(failure=failure):
                self.setUp()
                worker,document=self.worker,self.document
                worker.System.Guid=SimpleNamespace(Empty="empty")
                worker.Rhino.RhinoApp.RunScript=Mock()
                objects={key:SimpleNamespace(Id=key) for key in ["user","source"]}
                def insert(mesh):
                    self.assertEqual(mesh,"geometry")
                    if failure=="insert": return "empty"
                    objects["seed"]=SimpleNamespace(Id="seed")
                    return "seed"
                def delete(key,quiet):
                    self.assertNotIn(key,["user","source"])
                    self.assertTrue(quiet)
                    del objects[key]
                document.Objects=SimpleNamespace(AddMesh=insert,Delete=delete,UnselectAll=Mock())
                def run(script,verify):
                    self.assertEqual(script,"_MeshToNURB _TrimTriangularFaces=No _SelID seed _Enter")
                    self.assertTrue(verify)
                    for key in ["output-0","output-1"]: objects[key]=SimpleNamespace(Id=key)
                    if failure=="command": raise ValueError("partial output failure")
                with patch.object(worker,"_run_surface_script",side_effect=run) as command:
                    args=(document,lambda:list(objects.values()),"geometry","_MeshToNURB _TrimTriangularFaces=No")
                    if failure:
                        with self.assertRaises(ValueError): worker._seed_mesh_conversion_options(*args)
                    else: worker._seed_mesh_conversion_options(*args)
                    if failure=="insert": command.assert_not_called()
                self.assertEqual(set(objects),{"user","source"})
                document.Objects.UnselectAll.assert_called_once()
                worker.Rhino.RhinoApp.RunScript.assert_called_once_with("!",False)

    def test_mesh_conversion_scripts_validate_unsupported_choices_and_session_seeds(self):
        mesh=dict(sources=[dict(type="mesh")])
        for trim in [None,False,True]:
            for ngons in [None,False,True]:
                expected="_MeshToNURB"
                if trim is not None: expected+=" _TrimTriangularFaces="+("Yes" if trim else "No")
                if ngons is not None: expected+=" _UseNgons="+("Yes" if ngons else "No")
                self.assertEqual(self.worker._conversion_arguments(dict(mesh,trim_triangular_faces=trim,use_ngons=ngons),"MeshToNURB",None)[3],expected)
        for changes in [dict(delete_input=False),dict(delete_input=True),dict(use_ngons=1),dict(trim_triangular_faces=1),dict(sources=[dict(type="point")])]:
            with self.assertRaises(ValueError): self.worker._conversion_arguments(dict(mesh,**changes),"MeshToNURB",None)
        step=dict(mesh,command="MeshToNURB",trim_triangular_faces=False,use_ngons=True)
        for option in ["trim_triangular_faces","use_ngons"]:
            with patch.object(self.worker,"_geometry_conversion") as run,self.assertRaises(ValueError):
                self.worker._conversion_session(dict(steps=[dict(step,**{option:None})]),{})
            run.assert_not_called()
        with patch.object(self.worker,"_geometry_conversion",return_value=({},0)) as run:
            self.worker._conversion_session(dict(steps=[step,dict(mesh,command="MeshToNURB")]),{})
            self.assertEqual(run.call_count,2)

    def test_nurbs_scripts_include_bounded_mesh_options_and_validate_geometry(self):
        for delete in [None,False,True]:
            for trim in [None,False,True]:
                op=dict(sources=[dict(type="mesh")],delete_input=delete,trim_triangular_faces=trim)
                expected="_ToNURBS"+(" _DeleteInputObjects="+("Yes" if delete else "No") if delete is not None else "")
                if trim is not None: expected+=" _MeshOptions _TrimTriangularFaces="+("Yes" if trim else "No")+" _Enter"
                self.assertEqual(self.worker._conversion_arguments(op,"ToNURBS",None)[3],expected+" _Enter")
        for command,sources,trim in [("ToNURBS",[dict(type="line")],True),("ToNURBS",[dict(type="mesh")],1),("ConvertToBeziers",[dict(type="nurbs")],True)]:
            with self.assertRaises(ValueError): self.worker._conversion_arguments(dict(sources=sources,delete_input=False,trim_triangular_faces=trim),command,None)

    def test_nurbs_sessions_require_real_option_seed_and_explicit_first_mesh_choice(self):
        line=dict(command="ToNURBS",sources=[dict(type="line")],delete_input=False)
        mesh=dict(command="ToNURBS",sources=[dict(type="mesh")],delete_input=None)
        for steps in [[dict(line,sources=[dict(type="nurbs")])],[dict(line,sources=[dict(type="ellipse")])],[line,mesh]]:
            with patch.object(self.worker,"_geometry_conversion") as run,self.assertRaises(ValueError):
                self.worker._conversion_session(dict(steps=steps),{})
            run.assert_not_called()
        steps=[line,dict(mesh,trim_triangular_faces=True),mesh]
        with patch.object(self.worker,"_geometry_conversion",return_value=({},0)) as run:
            self.worker._conversion_session(dict(steps=steps),{})
            self.assertEqual(run.call_count,3)

    def test_script_builder_matches_observed_toggle_and_direction_prompts(self):
        for direction in [None,"U","V","Both"]:
            for delete in [None,False,True]:
                operation=dict(sources=[dict(type="surface")],delete_input=delete)
                script=self.worker._conversion_arguments(operation,"ConvertToSingleSpans",direction)[3]
                expected="_ConvertToSingleSpans"+(" _Direction _"+direction if direction else "")
                expected+=(" _DeleteInput="+("Yes" if delete else "No") if delete is not None else "")+" _Enter"
                self.assertEqual(script,expected)
        operation=dict(sources=[dict(type="surface")],delete_input=True,toggles=3)
        self.assertEqual(self.worker._conversion_arguments(operation,"ConvertToSingleSpans","U")[3],
                         "_ConvertToSingleSpans _Direction _U _DeleteInput=Yes _Toggle _Toggle _Toggle _Enter")

    def test_invalid_commands_options_and_ineligible_sources_fail_before_document_access(self):
        valid=dict(sources=[dict(type="surface")],delete_input=False)
        cases=[(valid,"_Delete",None),(valid,"ConvertToSingleSpans","U _Delete"),
               (valid,"ConvertToBeziers","U"),(dict(valid,undo_after=1),"ConvertToSingleSpans",None),
               (dict(valid,sources=[dict(type="nurbs")]),"ConvertToSingleSpans",None),
               (dict(valid,toggles=True),"ConvertToSingleSpans","U"),
               (dict(valid,toggles=4),"ConvertToSingleSpans","U"),
               (dict(valid,toggles=1),"ConvertToSingleSpans","Both"),
               (dict(valid,toggles=1),"ConvertToBeziers",None)]
        for op,command,direction in cases:
            with self.subTest(command=command,direction=direction),self.assertRaises(ValueError):
                self.worker._geometry_conversion(op,{},command,direction)

    def test_each_command_must_be_seeded_and_all_steps_are_validated_before_execution(self):
        b=dict(command="ConvertToBeziers",sources=[dict(type="nurbs")],delete_input=False)
        s=dict(command="ConvertToSingleSpans",sources=[dict(type="surface")],delete_input=True,direction="U")
        for steps in [[],[b]*33,[dict(b,delete_input=None)],[b,dict(s,delete_input=None)],
                      [b,dict(s,direction=None)],[b,dict(b,command="Delete")],
                      [b,dict(b,sources=[dict(type="point")])],[b,dict(b,selected=[])],
                      [dict(s,direction="Both"),dict(s,direction=None,toggles=1)]]:
            with self.subTest(steps=steps),patch.object(self.worker,"_geometry_conversion") as run:
                with self.assertRaises(ValueError): self.worker._conversion_session(dict(steps=steps),{})
                run.assert_not_called()

    def test_ordered_steps_forward_missing_options_and_undo_without_reseeding(self):
        steps=[dict(command="ConvertToBeziers",sources=[dict(type="nurbs")],delete_input=True,undo_after=True),
               dict(command="ConvertToSingleSpans",sources=[dict(type="surface")],delete_input=False,direction="V"),
               dict(command="ConvertToBeziers",sources=[dict(type="nurbs")],delete_input=None),
               dict(command="ConvertToSingleSpans",sources=[dict(type="surface")],delete_input=None,direction=None)]
        with patch.object(self.worker,"_geometry_conversion",side_effect=[(dict(step=i),0) for i in range(4)]) as run:
            value,elapsed=self.worker._conversion_session(dict(steps=steps),{})
            self.assertEqual(elapsed,0);self.assertEqual(value,dict(states=[dict(step=i) for i in range(4)]))
            for call,step in zip(run.call_args_list,steps):
                self.assertEqual(call.args,(step,{},step["command"],step.get("direction")))
