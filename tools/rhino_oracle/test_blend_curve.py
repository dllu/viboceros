"""Adjustable blend results, source ownership and complete replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_evidence as evidence,report_blend_curve as report
from .check_blend_curve import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class BlendCurveTests(unittest.TestCase):
    def test_all_observations_results_samples_and_diagnostics_are_retained(self):
        w,d,c,picks,stations,paths=evidence.collect(PROOF)
        self.assertEqual((len(w),len(d),len(c),picks,stations),(25,9,4,52,759))
        self.assertGreater(len(paths),100)
        self.assertEqual(sum(evidence.command_result(v)=='Nothing'for v in w.values()),23)
        self.assertTrue(all(v['success']is False for v in w.values()))
        self.assertEqual(sum(reason=='input driver timeout'for reason in d.values()),3)

    def test_fixed_private_unique_requests_reject_invalid_input_before_launch(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                for iterations in [True,1.0,0,2]:
                    v=copy.deepcopy(request);v['iterations']=iterations
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleBlendInvalid').run_rhino(v)
                v=copy.deepcopy(request);v['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleBlendInvalid').run_rhino(v)
                for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
                    with self.assertRaises(ValueError):evidence.recipes.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_nonfinite_geometry_result_reclassification_and_diagnostic_promotion_fail(self):
        capture=evidence.captures
        def changed(mutator):
            rows,captures,paths=capture(PROOF);mutator(rows);return rows,captures,paths
        first='blend_curve_pilot/adjustable_default'
        mutations=[
            (lambda rows:evidence.output(rows[first])['samples'][0]['point'].__setitem__(0,float('nan')),'invalid native blend geometry'),
            (lambda rows:rows[first].update(success=True),'native result semantics changed'),
            (lambda rows:rows[first]['prescribed_case']['actions'][0]['pick'].__setitem__(0,-.25001),'BlendCrv recipe changed'),
            (lambda rows:rows[first]['input_trace'][0].update(completed=0),'incomplete BlendCrv workflow'),
            (lambda rows:evidence.output(rows['blend_curve_discovery/g3_lines'])['curve'].update(degree=5),'higher continuity classification changed'),
            (lambda rows:rows['blend_curve_holdouts/g2_handle_start']['input_trace'][0].update(errors=[]),'driver diagnostic classification changed'),
        ]
        for mutation,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutation)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
        def corrupt_join(rows):
            v=rows['blend_curve_discovery/join_defaults']
            for phase in ['after','redo']:
                next(o for o in v[phase]if o['native_type']=='PolyCurve')['attributes']['groups']=[]
        with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(corrupt_join)):
            with self.assertRaisesRegex(ValueError,'joined blend ownership changed'):evidence.collect(PROOF)

    def test_capture_and_failed_dispatch_receipts_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';original=path.read_bytes();rows=json.loads(original);rows[0]['exit_status']=1;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete BlendCrv capture'):evidence.collect(proof)
            path.write_bytes(original);path=proof/'attempts.json';rows=json.loads(path.read_bytes());rows[0]['exit_status']=0;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'failed dispatch attempt changed'):evidence.collect(proof)

    def test_record_reconstruction_requires_finite_complete_replays(self):
        path=ROOT/'docs/blend-curve-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'blend-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('BLEND_CURVE_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_sample_error',float('nan'),'geometry gate'),('native_stations',0,'blend replay classification'),('accepted',not row['accepted'],'blend replay classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()
