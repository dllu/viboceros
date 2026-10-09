"""Owned control inputs wait for native getter readiness and reject foreign data."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from .block_edit_controls_input import BlockEditController
from .client import OracleProtocolError


class BlockEditControlInputTests(unittest.TestCase):
    def controller(self):
        return BlockEditController({'operations': [{'id': 'case', 'steps': [
            {'action': 'edit_roundtrip', 'base_point': [1, 2, 3]}]}]})

    def marker(self):
        return dict(token='case-0-base_point', action='base_point',
                    values=[1, 2, 3], button=[299, 183])

    def test_input_waits_for_the_owned_getter_and_is_delivered_once(self):
        controller = self.controller()
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            (job / 'block-edit-control.json').write_text(json.dumps(self.marker()))
            with patch('tools.rhino_oracle.block_edit_controls_input._rhino_window_for_pids', return_value='owned'), patch('tools.rhino_oracle.block_edit_controls_input.subprocess.run') as run, patch('tools.rhino_oracle.block_edit_controls_input.time.sleep'):
                run.return_value.stdout='X=0\nY=18\nWIDTH=1920\nHEIGHT=1062\n'
                controller(job, {123})
                self.assertEqual(run.call_count, 1)  # Button click only.
                controller(job, {123})
                self.assertEqual(run.call_count, 1)
                (job / 'block-edit-control.json.ready').write_text(json.dumps(dict(token='case-0-base_point', prompt='New base point')))
                controller(job, {123})
                calls = run.call_count
                self.assertGreater(calls, 1)
                self.assertFalse((job / 'block-edit-control.json.ack').exists())
                controller.submitted['case-0-base_point'] -= 2
                (job / 'block-edit-control.json.active').write_text(json.dumps(dict(token='case-0-base_point', prompt='Command')))
                controller(job, {123})
                self.assertEqual(run.call_count, calls)
                (job / 'block-edit-control.json.active').write_text(json.dumps(dict(token='case-0-base_point', prompt='New base point')))
                controller(job, {123})
                self.assertGreater(run.call_count, calls)
                calls = run.call_count
                (job / 'block-edit-control.json.completed').write_text(json.dumps('case-0-base_point'))
                controller(job, {123})
                self.assertEqual(run.call_count, calls)
                self.assertEqual(json.loads((job / 'block-edit-control.json.ack').read_text()), 'case-0-base_point')
                controller(job, {123})
                self.assertEqual(run.call_count, calls)
                controller.record_diagnostics({})

    def test_foreign_request_and_getter_tokens_fail_without_keyboard_input(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            controller = self.controller()
            value = self.marker()
            value['token'] = 'foreign'
            (job / 'block-edit-control.json').write_text(json.dumps(value))
            with patch('tools.rhino_oracle.block_edit_controls_input.subprocess.run') as run:
                with self.assertRaises(OracleProtocolError):
                    controller(job, {123})
                run.assert_not_called()
            value = self.marker()
            (job / 'block-edit-control.json').write_text(json.dumps(value))
            controller.clicked[value['token']] = value
            (job / 'block-edit-control.json.ready').write_text(json.dumps(dict(token='foreign', prompt='New base point')))
            with patch('tools.rhino_oracle.block_edit_controls_input._rhino_window_for_pids', return_value='owned'), patch('tools.rhino_oracle.block_edit_controls_input.subprocess.run') as run:
                with self.assertRaises(OracleProtocolError):
                    controller(job, {123})
                run.assert_not_called()

class BlockEditContextInputTests(unittest.TestCase):
    def test_active_context_acknowledgement_does_not_click_and_counts_toward_completion(self):
        controller=BlockEditController({'operations':[{'id':'case','steps':[{'action':'edit_roundtrip','contexts':[{'definition':'part','translation':[0,0,0]}]}]}]})
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory);token='case-0-context-0'
            (job/'block-edit-context.json').write_text(json.dumps(dict(token=token,definition='part',point=[0,0],skip=True)))
            with patch('tools.rhino_oracle.block_edit_controls_input._rhino_window_for_pids',return_value='owned'),patch('tools.rhino_oracle.block_edit_controls_input.subprocess.run') as run:
                controller(job,{123});run.assert_not_called()
            self.assertEqual(json.loads((job/'block-edit-context.json.ack').read_text()),token)
            controller.record_diagnostics({})

    def test_foreign_context_token_is_rejected_before_any_pointer_input(self):
        controller=BlockEditController({'operations':[{'id':'case','steps':[{'action':'edit_roundtrip','contexts':[{'definition':'child','translation':[1,2,3]}]}]}]})
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory)
            (job/'block-edit-context.json').write_text(json.dumps(dict(token='foreign',definition='child',point=[100,200],skip=False)))
            with patch('tools.rhino_oracle.block_edit_controls_input.subprocess.run') as run:
                with self.assertRaises(OracleProtocolError):controller(job,{123})
                run.assert_not_called()
