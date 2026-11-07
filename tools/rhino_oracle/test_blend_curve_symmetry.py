"""Shift input evidence, native precision and complete replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_symmetry_evidence as evidence,report_blend_curve_symmetry as report
from .check_blend_curve_symmetry import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class SymmetryBlendTests(unittest.TestCase):
    def test_every_native_workflow_modifier_and_ownership_is_retained(self):
        rows,captures,picks,shift,stations,derivatives,joins,paired,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures),picks,shift,stations,derivatives,joins,paired),(33,4,104,32,1089,483,3,23))
        self.assertGreater(len(paths),1300)
        self.assertEqual(evidence.length_measurements(rows),3)
        baseline=evidence.handles(evidence.prior.output(rows['blend_curve_symmetry_pilot/symmetric_shape_baseline']))
        paired=evidence.handles(evidence.prior.output(rows['blend_curve_symmetry_pilot/symmetric_shape_start']))
        self.assertEqual(baseline[0],paired[0]);self.assertNotEqual(baseline[1],paired[1]);self.assertEqual(paired,[3.,3.])
        independent=evidence.handles(evidence.prior.output(rows['blend_curve_symmetry_discovery/symmetric_then_independent']))
        self.assertEqual(independent,[3.,4.])

    def test_changed_modifier_inputs_paired_geometry_and_stations_fail(self):
        capture=evidence.captures
        def changed(mutate):
            rows,c,p=capture(PROOF);mutate(rows);return rows,c,p
        first='blend_curve_symmetry_pilot/symmetric_shape_start'
        def paired(rows):
            for phase in ['after','redo']:rows[first][phase][-1]['curve']['control_points'][-2]['point'][0]+=1
        def derivative(rows):
            for phase in ['before','after','undo','redo']:rows[first][phase][1]['derivative_samples'].pop()
        mutations=[
            (lambda r:r[first]['input_trace'][0]['clicks'][-1].update(shift_key_down=False),'public Shift input receipt'),
            (lambda r:r[first]['input_trace'][0].update(completed=0),'incomplete symmetry workflow'),
            (lambda r:r[first].update(success=True),'native symmetry result'),
            (lambda r:r[first]['after'][-1]['samples'][0]['point'].__setitem__(0,float('nan')),'invalid native symmetry geometry'),
            (paired,'paired handle lengths'),(derivative,'missing symmetry derivative'),
        ]
        for mutate,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_public_lengths_explain_the_default_domain_and_tight_convergence(self):
        rows,_,_=evidence.captures(PROOF);self.assertEqual(evidence.length_measurements(rows),3)
        key='blend_curve_symmetry_lengths/symmetric_unequal_rational_lengths';o=evidence.prior.output(rows[key]);m=o['length_measurements']
        self.assertGreater(m['default'],600.);self.assertGreater(abs(m['default']-m['fractional'][-1]['length']),2e-6)
        self.assertLess(abs(m['default']-m['fractional'][-1]['length']),1e-8*m['default'])
        m['fractional'][1]['length']+=1e-4
        with self.assertRaisesRegex(ValueError,'fractional length measurement'):evidence.length_measurements(rows)

    def test_terminal_receipts_and_fixed_unique_private_requests_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';rows=json.loads(p.read_bytes());rows[0]['exit_status']=1;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                request=copy.deepcopy(request);request['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSymmetryInvalid').run_rhino(request)
            launch.assert_not_called()

    def test_pinned_record_requires_every_finite_replay_and_both_length_gates(self):
        p=ROOT/'docs/blend-curve-symmetry-validation.json'
        if not p.exists():self.skipTest('record follows implementation commit')
        value=json.loads(p.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'symmetry-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('BLEND_CURVE_SYMMETRY_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_sample_error',float('nan'),'geometry gate'),('maximum_normalized_derivative_error',1.1e-6,'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain gate'),('maximum_parameter_domain_error',1.,'fractional domain gate'),('maximum_tight_length_error',1.1e-7,'tight length measurement'),('derivative_stations',99,'classification'),('accepted',False,'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_SYMMETRY_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()
