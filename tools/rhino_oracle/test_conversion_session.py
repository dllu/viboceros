"""Self-seeded option probes cannot depend on unrelated Rhino session history."""
from unittest.mock import Mock,patch
import unittest
from . import test_worker


class ConversionSessionTests(unittest.TestCase):
    def setUp(self): test_worker.RhinoWorkerTests.setUp(self)

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
