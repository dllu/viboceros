"""Reject malformed or unowned document edits before launching Rhino."""
import copy
import os
from pathlib import Path
import tempfile
from unittest import TestCase, mock
from .client import OracleClient, OracleError
from .reparameterize_face_probe import request, validate
from .structural_sdk import stage_helpers, validate_operations


class ReparameterizeFaceProbeTests(TestCase):
    def test_recipes_stage_the_complete_idle_command_worker(self):
        q=request('/tmp/owned-face-charts')
        self.assertEqual(len(q['operations']),30)
        validate_operations(q)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(q,Path(__file__).parent,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                {'reparameterize_face_probe.py','join_probe.py','merge_edges_probe.py'})

    def test_invalid_domains_and_unowned_execution_are_rejected_before_launch(self):
        q=request('/tmp/owned-face-charts');original=q['operations'][0]
        for domains in (True, 'Other', [[0.,0.],[0.,1.]], [[0.,float('nan')],[0.,1.]], [[0.,1.]], [[0.,1.],[False,1.]]):
            op=copy.deepcopy(original);op['domains']=domains
            with self.subTest(domains=domains),self.assertRaises(ValueError):validate(op)
        with (mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}),
              mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()),
              mock.patch('tools.rhino_oracle.client._run_logged') as launch):
            with self.assertRaisesRegex(OracleError, 'Face reparameterization'):OracleClient().run_rhino(q)
            q['iterations']=2
            with self.assertRaisesRegex(OracleError, 'Face reparameterization'):OracleClient(settings_scheme='VibocerosOracleFaceProbe').run_rhino(q)
            launch.assert_not_called()
