"""Higher-order evidence admission, diagnostics, and reconstruction gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_high_evidence as evidence, report_blend_curve_high as report
from .check_blend_curve_high import ROOT, PROOF
from .client import OracleClient, OracleProtocolError

class HigherBlendTests(unittest.TestCase):
    def test_capture_inventory_and_public_derivatives(self):
        rows,captures,picks,stations,derivatives,zero_ends,paths = evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures),picks,stations,derivatives,zero_ends),(25,2,56,825,279,20))
        self.assertGreater(len(paths),900)
        audit = report.measure(evidence.prior.output(rows['blend_curve_holdouts/g3_arc'])['curve'])
        self.assertAlmostEqual(audit['independent_length'],10.806095637284502,places=10)
        self.assertGreater(audit['native_domain_minus_length'],1e-6)
        self.assertLess(audit['native_domain_minus_length'],2e-6)

    def test_requests_require_private_unique_settings_before_launch(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen') as launch:
            for names in evidence.FAMILIES.values():
                request = evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError): OracleClient().run_rhino(request)
                changed = copy.deepcopy(request)
                changed['operations'] *= 2
                with self.assertRaises(OracleProtocolError): OracleClient(settings_scheme='VibocerosOracleHighInvalid').run_rhino(changed)
            launch.assert_not_called()

    def test_native_result_input_and_fourth_derivative_corruption_fail(self):
        original = evidence.captures
        key = 'blend_curve_high_discovery/high_quartic_g4'
        def change(mutate):
            rows,captures,paths = original(PROOF)
            mutate(rows[key])
            return rows,captures,paths
        def missing(v):
            for phase in ['before','undo']:
                next(o for o in v[phase] if o['attributes']['name']=='guide0')['derivative_samples'].pop()
        mutations = [
            (lambda v:v.update(success=True),'native result semantics'),
            (lambda v:v['input_trace'][0].update(completed=0),'incomplete higher-order'),
            (missing,'missing native fourth-order'),
            (lambda v:evidence.prior.output(v)['derivative_samples'][0]['derivatives'][4].__setitem__(0,float('nan')),'invalid native high-order'),
        ]
        for mutate,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:change(mutate)):
                with self.assertRaisesRegex(ValueError,message): evidence.collect(PROOF)
        # Keep redo consistent so this independently tests the Native G4 policy.
        def nonzero(v):
            for phase in ['after','redo']:
                next(o for o in v[phase] if o['attributes']['name']=='')['derivative_samples'][0]['derivatives'][4][0]=1.
        with mock.patch.object(evidence,'captures',side_effect=lambda *a:change(nonzero)):
            with self.assertRaisesRegex(ValueError,'native zero-fourth'): evidence.collect(PROOF)

    def test_capture_exit_status_is_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';v=json.loads(path.read_bytes());v[0]['exit_status']=1;path.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'): evidence.collect(proof)

    def test_record_reconstruction_and_replay_gates(self):
        path=ROOT/'docs/blend-curve-high-validation.json'
        if not path.exists(): self.skipTest('record follows implementation commit')
        record=json.loads(path.read_bytes());revision=record['implementation_revision']
        self.assertEqual(record,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'blend-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines) if line.startswith('BLEND_CURVE_UI_HIGH '))
            row=json.loads(lines[i].split(' ',1)[1])
            for field,value,message in [
                ('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),
                ('maximum_native_sample_error',float('nan'),'geometry gate'),
                ('maximum_normalized_derivative_error',1.1e-6,'geometry gate'),
                ('maximum_parameter_domain_error',2.1e-6,'parameter or derivative'),
                ('maximum_absolute_derivative_error',float('nan'),'parameter or derivative'),
                ('native_stations',0,'classification'),('derivative_stations',99,'classification'),
                ('accepted',False,'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_UI_HIGH '+json.dumps(dict(row,**{field:value}))+'\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message): report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'): report.build(proof,revision)

if __name__=='__main__': unittest.main()
