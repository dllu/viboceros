"""Immutable plane input, source/history checks and owned physical input."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from . import surface_interpolate_plane_cases as cases
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
from .surface_interpolate_plane_input import PlaneKeyboard

HERE=Path(__file__).resolve().parent
FAMILIES=[('discovery',cases.request,11),('managed',cases.managed_request,4),
          ('bounded',cases.bounded_request,5),('workflows',cases.workflow_request,20),('audit',cases.audit_request,3)]
def observed(suffix):
    return json.loads(gzip.decompress((HERE/('observations/surface_interpolate_plane_'+suffix+'.json.gz')).read_bytes()))

class PlaneInterpolationTests(unittest.TestCase):
    def test_fixed_requests_and_all_frozen_producers(self):
        for suffix,factory,count in FAMILIES:
            raw=(HERE/('fixtures/surface_interpolate_plane_'+suffix+'.json')).read_bytes()
            request=json.loads(raw);self.assertEqual(request,factory())
            for op in request['operations']:cases.validate(op)
            native=observed(suffix);self.assertEqual(len(native['results']),count)
            self.assertEqual([r['id']for r in native['results']],[r['id']for r in request['operations']])
            self.assertTrue(all('error'not in r for r in native['results']))
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(raw).hexdigest())
            for name,digest in native['provenance']['sources'].items():
                archive=HERE/('observations/source_snapshots/'+digest+'.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(),digest,name)

    def test_native_sources_history_and_acknowledged_escape(self):
        completed=controls=escapes=0
        for suffix in ['workflows','audit']:
            for row in observed(suffix)['results']:
                v=row['value'];self.assertTrue(v['returned_idle'])
                self.assertTrue(v['host_input']['private_owned_window'])
                self.assertEqual(v['host_input']['escapes'],len(v['cancel_guard']))
                escapes+=v['host_input']['escapes']
                originals={o['attributes']['name']:o for o in v['before']['objects']}
                for phase in ['after','undo','redo']:
                    for obj in v[phase]['objects']:
                        if obj['role']!='output':self.assertEqual(obj,originals[obj['attributes']['name']])
                        else:
                            self.assertEqual(obj['selected'],0);self.assertTrue(obj['native_valid'])
                            self.assertEqual(obj['attributes']['layer'],'Interpolation plane output')
                            self.assertEqual(obj['attributes']['name'],'');self.assertEqual(obj['attributes']['groups'],[])
                if v['control']:
                    controls+=1;self.assertFalse(v['success']);self.assertEqual(v['after'],v['before'])
                else:
                    completed+=1;self.assertTrue(v['success'])
                    trace,=v['input_trace'];self.assertEqual(trace['errors'],[])
                    self.assertEqual(trace['completed'],trace['expected'])
                    self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
        self.assertEqual((completed,controls,escapes),(18,5,3))

    def test_native_patch_clamping_empty_document_and_relative_coordinate_space(self):
        rows={r['id'].split('audit_')[-1]:r['value'] for r in observed('audit')['results']}
        far=next(o for o in rows['far']['after']['objects']if o['role']=='output')['curve']['control_points']
        empty=next(o for o in rows['empty']['after']['objects']if o['role']=='output')['curve']['control_points']
        self.assertEqual(far[0]['point'],[-20.41,-20.41,0.]);self.assertEqual(far[-1]['point'],[20.41,-20.41,0.])
        self.assertEqual(empty[0]['point'],[-.01,-.01,0.]);self.assertEqual(empty[-1]['point'],[.01,-.01,0.])
        relative=next(o for o in rows['relative_yz']['after']['objects']if o['role']=='output')['curve']['control_points']
        self.assertTrue(all(p['point'][0]==0. and p['point'][2]==0. for p in relative))
        self.assertEqual(relative[-1]['point'],[0.,3.,0.])

    def test_only_fixed_bounded_private_requests_can_launch(self):
        request=cases.workflow_request()
        for change in [dict(id='foreign'),dict(case=[]),dict(extra='script')]:
            row=copy.deepcopy(request['operations'][0]);row.update(change)
            with self.assertRaises(ValueError):cases.validate(row)
        for value in [True,2,1.0]:
            q=copy.deepcopy(request);q['iterations']=value
            with self.assertRaises(OracleProtocolError):PlaneKeyboard(q)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
            spawn.assert_not_called()

    def test_escape_driver_uses_only_owned_windows_and_requires_matching_receipts(self):
        request=cases.workflow_request();request['operations']=request['operations'][-5:-4]
        driver=PlaneKeyboard(request);name=request['operations'][0]['id']
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);(job/'worker-progress.log').write_text('INTERPOLATION_PLANE_ESCAPE '+name+' 0\n')
            with mock.patch('tools.rhino_oracle.client._rhino_window_for_pids',return_value='owned-window'),mock.patch('tools.rhino_oracle.surface_interpolate_plane_input.subprocess.run')as send:
                driver(job,set());send.assert_not_called()
                driver(job,{123});driver(job,{123})
                send.assert_called_once_with(['xdotool','windowactivate','--sync','owned-window','key','--clearmodifiers','Escape'],check=True,timeout=10)
            response=dict(results=[dict(id=name,value=dict(returned_idle=True,cancel_guard=[dict(sequence=0)]))])
            driver.record_diagnostics(response);self.assertEqual(response['results'][0]['value']['host_input']['escapes'],1)
            response['results'][0]['value']['cancel_guard'][0]['sequence']=1
            with self.assertRaises(OracleProtocolError):driver.record_diagnostics(response)

    def test_staged_helper_closure(self):
        with tempfile.TemporaryDirectory()as directory:
            target=Path(directory);stage_helpers(cases.workflow_request(),HERE,target)
            for name in ['surface_interpolate_plane_probe','surface_interpolate_plane_cases','surface_interpolate_plane_input',
                         'conic_getter_input','managed_mouse_callback','snap_environment','number_token','join_probe','merge_edges_probe']:
                self.assertEqual((target/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())

    def test_validation_record_uses_preserved_evidence_and_keeps_the_gate(self):
        from .report_surface_interpolate_plane import ROOT,build
        record=json.loads((ROOT/'docs/surface-interpolation-planes-validation.json').read_text())
        self.assertRegex(record['implementation_revision'],r'^[0-9a-f]{40}$')
        self.assertEqual(record,build(ROOT/'docs/surface-interpolation-planes-proof',record['implementation_revision']))
        self.assertEqual(record['model_epsilon'],1e-6);self.assertEqual(record['qualified_workflows'],23)
        self.assertFalse(record['full_native_parity']);self.assertFalse(record['continuous_native_distance_qualified'])

if __name__=='__main__':unittest.main()
