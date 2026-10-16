"""Validate real host staging without launching Rhino or touching existing windows."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase,mock,skipUnless
from .client import OracleClient,OracleError
from .structural_sdk import PROBES,stage_helpers,validate_operations


class StructuralStagingTests(TestCase):
    def request(self,name):
        return json.loads(Path(__file__).with_name('fixtures').joinpath(name+'.json').read_text())

    def test_mixed_structural_sdk_helpers_import_from_the_actual_standalone_job(self):
        operations=[]
        for fixture in ('rail_revolve','two_rail_sweep','divide_along_creases','cull_degenerate_mesh_faces','geometry_validity','coplanar_faces','coplanar_mesh','selected_coplanar_faces'):
            operation=self.request(fixture)['operations'][0]
            operation['id']='case-'+str(len(operations))
            operations.append(operation)
        request=dict(operations=operations)
        validate_operations(request)
        before=copy.deepcopy(request)
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory)
            stage_helpers(request,Path(__file__).parent,job)
            self.assertEqual({p.name for p in job.iterdir()},
                {'rail_revolve_probe.py','two_rail_sweep_probe.py','divide_creases_probe.py','cull_degenerate_probe.py','geometry_validity_probe.py','coplanar_faces_probe.py','coplanar_mesh_probe.py','selected_coplanar_faces_probe.py'})
            subprocess.run([sys.executable,'-I','-c',
                'import sys; sys.path.insert(0,sys.argv[1]); import rail_revolve_probe, two_rail_sweep_probe, divide_creases_probe, cull_degenerate_probe, geometry_validity_probe, coplanar_faces_probe, coplanar_mesh_probe, selected_coplanar_faces_probe',
                str(job)],check=True,capture_output=True,text=True)
        self.assertEqual(request,before)

    @skipUnless(sys.platform.startswith("linux"), "Wine launcher host paths require Linux")
    def test_actual_client_stages_each_registered_probe_before_launch_and_cleans_up(self):
        for fixture in ('rail_revolve','two_rail_sweep','divide_along_creases','cull_degenerate_mesh_faces','geometry_validity','coplanar_faces','coplanar_mesh','selected_coplanar_faces'):
            request=self.request(fixture)
            paths=[]
            def stop_before_launch(command,cwd,timeout,log_path):
                job=log_path.parent;paths.append(job)
                specification=PROBES[request['operations'][0]['op']]
                for module in (specification[0],*specification[1]):
                    self.assertEqual((job/(module+'.py')).read_bytes(),Path(__file__).with_name(module+'.py').read_bytes())
                self.assertTrue((job/'rhino_worker.py').is_file())
                self.assertTrue((job/'request.json').is_file())
                raise OracleError('staging verified')
            with (self.subTest(fixture=fixture),
                  mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged',side_effect=stop_before_launch),
                  self.assertRaisesRegex(OracleError,'staging verified')):
                OracleClient(launcher='/bin/true').run_rhino(request)
            self.assertEqual(len(paths),1)
            self.assertFalse(paths[0].exists())

    @skipUnless(sys.platform.startswith("linux"), "Wine launcher host paths require Linux")
    def test_invalid_operations_and_local_mesh_subset_requests_fail_before_launch(self):
        cases=[]
        for fixture in ('rail_revolve','two_rail_sweep','divide_along_creases','cull_degenerate_mesh_faces','geometry_validity','coplanar_faces','selected_coplanar_faces'):
            request=self.request(fixture);request['operations'][0]['extra']=True;cases.append(request)
        cases.append(self.request('selected_coplanar_mesh_local'))
        for request in cases:
            with (self.subTest(operation=request['operations'][0]),
                  mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._run_logged') as launch,
                  self.assertRaises(ValueError)):
                OracleClient(launcher='/bin/true').run_rhino(request)
            launch.assert_not_called()
