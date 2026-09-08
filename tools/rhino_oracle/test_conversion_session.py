"""Self-seeded option probes cannot depend on unrelated Rhino session history."""
from unittest.mock import Mock,patch
from types import SimpleNamespace
import unittest
from . import test_worker


class ConversionSessionTests(unittest.TestCase):
    def setUp(self): test_worker.RhinoWorkerTests.setUp(self)

    def test_bezier_postselection_scripts_finish_with_one_boolean_answer_or_cancel(self):
        base=dict(sources=[dict(type="line"),dict(type="surface"),dict(type="mesh"),dict(type="brep",cap_surface={}),dict(type="point")],selected=[1,4,0,2,3])
        ids=["line","surface","mesh","solid","point"]
        for delete in [False,True,None]:
            answer="_Enter" if delete is None else "_Yes" if delete else "_No"
            for postselect in [False,True]:
                for cancel in [False,True]:
                    op=dict(base,delete_input=delete,postselect=postselect,cancel=cancel)
                    _,selected,_,script=self.worker._conversion_arguments(op,"ConvertToBeziers",None)
                    expected="_ConvertToBeziers " + ("_SelID surface _SelID line _Enter " if postselect else "") + ("!" if cancel else answer)
                    self.assertEqual(self.worker._conversion_selection_script(op,"ConvertToBeziers",script,ids,selected),expected)
        op=dict(base,postselect=True,cancel=True,cancel_at_selection=True,selected=[])
        _,selected,_,script=self.worker._conversion_arguments(op,"ConvertToBeziers",None)
        self.assertEqual(self.worker._conversion_selection_script(op,"ConvertToBeziers",script,ids,selected),"_ConvertToBeziers  !")

    def test_bezier_invalid_cancellation_paths_and_session_seeds_fail_before_document_access(self):
        base=dict(sources=[dict(type="line"),dict(type="mesh")],selected=[0],delete_input=True)
        for changes in [dict(postselect=1),dict(cancel=1),dict(cancel_at_selection=True),
                        dict(cancel=True,cancel_at_selection=True),dict(cancel=True,selected=[]),
                        dict(cancel=True,undo_after=True),dict(postselect=True,initial_selection=[0]),
                        dict(postselect=True,cancel=True,selected=[1]),
                        dict(postselect=True,cancel=True,cancel_at_selection=True,selected=[],sources=[dict(type="point")])]:
            with self.subTest(changes=changes),self.assertRaises(ValueError):
                self.worker._geometry_conversion(dict(base,**changes),{})
        for postselect in [False,True]:
            first=dict(base,command="ConvertToBeziers",cancel=True,postselect=postselect)
            with patch.object(self.worker,"_geometry_conversion") as run,self.assertRaises(ValueError):
                self.worker._conversion_session(dict(steps=[first]),{})
            run.assert_not_called()
        self.worker._conversion_arguments(dict(base,postselect=True,initial_selection=[1]),"ConvertToBeziers",None)

    def test_nurbs_postselection_scripts_keep_selection_confirmation_and_mesh_options_ordered(self):
        base=dict(sources=[dict(type="line"),dict(type="mesh"),dict(type="point")],selected=[1,2,0],
                  delete_input=False,trim_triangular_faces=False)
        for changes,expected in [
            ({},"_ToNURBS _DeleteInputObjects=No _MeshOptions _TrimTriangularFaces=No _Enter _Enter"),
            (dict(cancel=True),"_ToNURBS _DeleteInputObjects=No _MeshOptions _TrimTriangularFaces=No _Enter !"),
            (dict(postselect=True),"_ToNURBS _SelID mesh _SelID line _Enter _DeleteInputObjects=No _MeshOptions _TrimTriangularFaces=No _Enter _Enter"),
            (dict(postselect=True,cancel=True),"_ToNURBS _SelID mesh _SelID line _Enter _DeleteInputObjects=No _MeshOptions _TrimTriangularFaces=No _Enter !"),
            (dict(postselect=True,cancel=True,cancel_at_selection=True),"_ToNURBS _SelID mesh _SelID line !"),
        ]:
            op=dict(base,**changes)
            _,selected,_,script=self.worker._conversion_arguments(op,"ToNURBS",None)
            self.assertEqual(self.worker._conversion_selection_script(op,"ToNURBS",script,["line","mesh","point"],selected),expected)

    def test_nurbs_cancelled_or_noop_input_cannot_seed_memory_and_invalid_stages_fail_early(self):
        first=dict(command="ToNURBS",sources=[dict(type="line")],delete_input=True)
        for changes in [dict(cancel=True),dict(postselect=True,cancel=True),dict(sources=[dict(type="nurbs")])]:
            with patch.object(self.worker,"_geometry_conversion") as run,self.assertRaises(ValueError):
                self.worker._conversion_session(dict(steps=[dict(first,**changes)]),{})
            run.assert_not_called()
        for changes in [dict(cancel_at_selection=True),dict(cancel_at_selection=1),
                        dict(cancel=True,cancel_at_selection=True),dict(cancel=True,selected=[]),
                        dict(postselect=True,cancel=True,sources=[dict(type="nurbs")]),
                        dict(postselect=True,cancel=True,cancel_at_selection=True,selected=[],sources=[dict(type="point")]),
                        dict(postselect=True,initial_selection=[0])]:
            with self.subTest(changes=changes),self.assertRaises(ValueError):
                self.worker._geometry_conversion(dict(first,**changes),{},"ToNURBS")

    def test_mesh_postselection_and_cancel_preflight_before_document_access(self):
        valid=dict(sources=[dict(type="mesh"),dict(type="point")],selected=[0],
                   postselect=True,trim_triangular_faces=False,use_ngons=True)
        for changes in [dict(postselect=1),dict(cancel=1),dict(postselect=False,cancel=True),
                        dict(cancel=True,undo_after=True),dict(initial_selection=[0]),
                        dict(initial_selection=[1,1]),dict(initial_selection=[2]),
                        dict(initial_selection=[True]),dict(initial_selection="1"),
                        dict(postselect=False,initial_selection=[1]),dict(selected=[]),
                        dict(selected=[1]),dict(sources=[dict(type="point")],selected=[],cancel=True)]:
            with self.subTest(changes=changes),self.assertRaises(ValueError):
                self.worker._geometry_conversion(dict(valid,**changes),{},"MeshToNURB")
        for command in ["ToNURBS","ConvertToBeziers","ConvertToSingleSpans"]:
            with self.subTest(command=command),self.assertRaises(ValueError):
                self.worker._geometry_conversion(valid,{},command)
        for changes in [dict(initial_selection=[1]),dict(cancel=True),dict(cancel=True,selected=[])]:
            args=self.worker._conversion_arguments(dict(valid,**changes),"MeshToNURB",None)
            self.assertEqual(args[3],"_MeshToNURB _TrimTriangularFaces=No _UseNgons=Yes")

    def test_cancellation_can_seed_options_before_any_mesh_is_picked(self):
        first=dict(command="MeshToNURB",sources=[dict(type="mesh")],postselect=True,
                   cancel=True,selected=[],trim_triangular_faces=False,use_ngons=True)
        second=dict(command="MeshToNURB",sources=first["sources"])
        with patch.object(self.worker,"_geometry_conversion",return_value=({},0)) as run:
            self.worker._conversion_session(dict(steps=[first,second]),{})
            self.assertEqual(run.call_count,2)
            self.assertEqual(run.call_args_list[0].args,(first,{},"MeshToNURB",None))
            self.assertEqual(run.call_args_list[1].args,(second,{},"MeshToNURB",None))

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
