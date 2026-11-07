"""Reject incomplete endpoint capture and comparison evidence."""
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_empty_evidence as evidence, report_blend_curve_empty as report
from .check_blend_curve_empty import ROOT, PROOF
from .client import OracleClient, OracleProtocolError

class EndpointBlendTests(unittest.TestCase):
    def test_capture_inventory_and_input_receipts_are_reproducible(self):
        rows,captures,counts,paths = evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(27,2))
        self.assertEqual(counts,dict(physical_picks=77,accepted_workflows=26,cancelled_workflows=1,
                                     point_tangent_stations=858,derivative_stations=300,joined_workflows=21))
        self.assertGreater(len(paths),30)

    def test_incomplete_input_and_changed_untrimmed_source_are_rejected(self):
        original = evidence.captures
        def changed(mutate):
            rows,captures,paths = original(PROOF,evidence.FAMILIES)
            mutate(rows['blend_curve_empty_discovery/empty_first_trim'])
            return rows,captures,paths
        def source(value):
            for phase in ['after','redo']:
                row = next(o for o in value[phase] if o['attributes']['name'] == 'guide0')
                row['curve']['control_points'][0]['point'][0] += 1
        changes = [(lambda v:v['input_trace'][0].update(completed=0),'incomplete endpoint'),
                   (lambda v:v['input_trace'][0]['clicks'][0].update(click_acknowledged=False),'public input receipt'),
                   (source,'zero-side trim changed a source')]
        for mutate,message in changes:
            with mock.patch.object(evidence,'captures',side_effect=lambda *args:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):
                    evidence.collect(PROOF)

    def test_missing_nonfinite_and_out_of_gate_replays_are_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof = Path(directory)/'proof'
            shutil.copytree(PROOF,proof)
            path = proof/'empty-command-portable.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i,line in enumerate(lines) if line.startswith('BLEND_CURVE_EMPTY_COMMAND '))
            row = json.loads(lines[index].split(' ',1)[1])
            changes = [('maximum_native_sample_error',1.1e-6,'geometry gate'),
                       ('maximum_normalized_derivative_error',float('nan'),'geometry gate'),
                       ('maximum_parameter_domain_gate_ratio',1.1,'domain or derivative'),
                       ('maximum_parameter_domain_error',1.,'domain or derivative'),
                       ('maximum_absolute_derivative_error',float('inf'),'domain or derivative'),
                       ('native_stations',0,'classification'),('archives',0,'archive gate')]
            for field,error,message in changes:
                modified = lines.copy()
                modified[index] = 'BLEND_CURVE_EMPTY_COMMAND '+json.dumps(dict(row,**{field:error}))+'\n'
                path.write_bytes(gzip.compress(''.join(modified).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):
                    report.comparisons(proof)
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):
                report.comparisons(proof)

    def test_new_cases_require_private_settings_and_single_recipes(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}), mock.patch('subprocess.Popen') as launch:
            query = evidence.recipes.request(list(evidence.recipes.EMPTY_DISCOVERY))
            with self.assertRaises(OracleProtocolError):
                OracleClient().run_rhino(query)
            query['operations'] *= 2
            with self.assertRaises(OracleProtocolError):
                OracleClient(settings_scheme='VibocerosOracleEmptyInvalid').run_rhino(query)
            launch.assert_not_called()

    def test_pinned_record_reproduces(self):
        path = ROOT/'docs/blend-curve-empty-validation.json'
        if not path.exists():
            self.skipTest('record follows implementation commit')
        value = json.loads(path.read_bytes())
        self.assertEqual(value,report.build(PROOF,value['implementation_revision']))

if __name__ == '__main__':
    unittest.main()
