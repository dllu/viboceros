"""Point endpoint capture, calibration, ownership and complete replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_points_evidence as evidence,report_blend_curve_points as report
from .check_blend_curve_points import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class PointBlendTests(unittest.TestCase):
    def test_all_native_observations_diagnostics_and_attempts_are_retained(self):
        w,d,c,picks,stations,derivatives,joins,cancelled,paths=evidence.collect(PROOF)
        self.assertEqual((len(w),len(d),len(c),picks,stations,derivatives,joins,cancelled),(41,1,6,48,1254,441,12,3))
        self.assertGreater(len(paths),1000)
        self.assertEqual(sum(evidence.point_index(v)is None for v in w.values()),3)
        self.assertEqual(sum(evidence.command_result(v)=='Nothing'for v in w.values()),38)

    def test_public_calibration_uses_inputs_independently_of_outputs(self):
        rows,_,_=evidence.captures(PROOF)
        self.assertEqual(evidence.calibrations(rows),(3,1))
        natural=rows['blend_curve_point_pick_context/spatial_natural_g4']
        receipt=natural['input_trace'][0]['clicks'][0]['pick_context']['references'][0]['point']
        self.assertNotEqual(receipt,evidence.output(natural)['curve']['control_points'][0]['point'])
        for v in rows.values():
            for o in v['after']:
                if 'curve'in o:o['curve']['control_points'][0]['point']=[900.,900.,900.]
        self.assertEqual(evidence.calibrations(rows),(3,1))
        rows,_,_=evidence.captures(PROOF)
        rows['blend_curve_point_holdouts/point_reverse_spatial_g4']['input_trace'][0]['clicks'][0]['pixel'][0]+=1
        with self.assertRaisesRegex(ValueError,'missing exact spatial'):evidence.calibrations(rows)

    def test_corrupted_input_result_identity_and_derivatives_fail(self):
        original=evidence.captures
        def change(mutator):
            rows,c,p=original(PROOF);mutator(rows);return rows,c,p
        key='blend_curve_point_pilot/point_curve_to_point'
        mutations=[
            (lambda r:r[key].update(success=True),'native point result'),
            (lambda r:r[key]['input_trace'][0].update(completed=0),'incomplete point workflow'),
            (lambda r:evidence.output(r[key])['samples'][0]['point'].__setitem__(0,float('nan')),'invalid native point geometry'),
            (lambda r:r['blend_curve_point_pilot/point_two_points']['input_trace'][0].update(errors=[]),'two-point selector diagnostic'),
        ]
        for mutate,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:change(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
        def missing(r):
            v=r['blend_curve_point_discovery/point_end_g4']
            for phase in ['before','undo']:v[phase][1]['derivative_samples'].pop()
        with mock.patch.object(evidence,'captures',side_effect=lambda *a:change(missing)):
            with self.assertRaisesRegex(ValueError,'missing point derivative'):evidence.collect(PROOF)
        def ownership(r):
            v=r['blend_curve_point_modes/point_mixed_first_interior_join']
            for phase in ['after','redo']:next(o for o in v[phase]if o['native_type']=='PolyCurve')['attributes']['groups']=[]
        with mock.patch.object(evidence,'captures',side_effect=lambda *a:change(ownership)):
            with self.assertRaisesRegex(ValueError,'point join ownership'):evidence.collect(PROOF)

    def test_capture_status_and_interrupted_attempt_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';original=path.read_bytes();rows=json.loads(original);rows[0]['exit_status']=1;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)
            path.write_bytes(original);path=proof/'attempts.json';rows=json.loads(path.read_bytes());rows[0]['exit_status']=0;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'missing interrupted point'):evidence.collect(proof)

    def test_fixed_private_unique_requests_are_validated_before_launch(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                request=copy.deepcopy(request);request['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePointInvalid').run_rhino(request)
            launch.assert_not_called()

    def test_record_reconstruction_requires_every_finite_replay(self):
        path=ROOT/'docs/blend-curve-points-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'point-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('BLEND_CURVE_POINT_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_sample_error',float('nan'),'geometry gate'),('maximum_normalized_derivative_error',1.1e-6,'geometry gate'),('maximum_parameter_domain_error',2.1e-6,'parameter or derivative'),('derivative_stations',99,'classification'),('accepted',not row['accepted'],'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_POINT_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()
