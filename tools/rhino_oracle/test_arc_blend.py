"""Fixed ArcBlend capture gates, provenance and reconstructed geometry coverage."""
import copy
import gzip
import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_evidence as evidence, report_arc_blend as report
from .check_arc_blend import ROOT, PROOF
from .client import OracleClient, OracleProtocolError
from .arc_blend_evidence import read

class ArcBlendTests(unittest.TestCase):
    def test_all_native_observations_and_frozen_producers_are_classified(self):
        matched, rejected, diagnostics, workflows, captures, paths = evidence.collect(PROOF)
        self.assertEqual((len(matched), len(rejected), len(diagnostics), len(workflows), len(captures)), (20, 17, 10, 6, 4))
        self.assertGreater(len(paths), 20)
        for family in evidence.FAMILIES:
            data = read(family)
            for digest in data['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((evidence.HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())).hexdigest(), digest)

    def test_only_fixed_private_unique_bounded_inputs_can_launch(self):
        with mock.patch.dict('os.environ', {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), mock.patch('subprocess.Popen') as launch:
            for module, names in evidence.FAMILIES.values():
                request = module.request(names)
                with self.assertRaises(OracleProtocolError):
                    OracleClient().run_rhino(request)
                for iterations in [True, 1.0, 0, 2]:
                    changed = copy.deepcopy(request)
                    changed['iterations'] = iterations
                    with self.assertRaises(OracleProtocolError):
                        OracleClient(settings_scheme='VibocerosOracleArcBlendInvalid').run_rhino(changed)
                changed = copy.deepcopy(request)
                changed['operations'].append(changed['operations'][0])
                with self.assertRaises(OracleProtocolError):
                    OracleClient(settings_scheme='VibocerosOracleArcBlendInvalid').run_rhino(changed)
                changed = copy.deepcopy(request)
                changed['operations'] *= 65
                with self.assertRaises(OracleProtocolError):
                    OracleClient(settings_scheme='VibocerosOracleArcBlendInvalid').run_rhino(changed)
                for mutation in [dict(case=None), dict(case=[]), dict(case='foreign'), dict(id='foreign'), dict(script='Line')]:
                    operation = dict(request['operations'][0], **mutation)
                    with self.assertRaises(ValueError):
                        module.validate(operation)
            launch.assert_not_called()

    def test_raw_direction_and_incomplete_input_diagnostics_cannot_be_hidden(self):
        def changed(family):
            data = read(family)
            if family == 'arc_blend_sdk':
                row = next(r for r in data['results'] if r['id'] == 'spatial_1')
                row['value']['segments'][1]['samples'][0]['tangent'] = row['value']['segments'][0]['samples'][-1]['tangent']
            return data
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'raw-vector native diagnostic changed'):
                evidence.collect(PROOF)
        def changed(family):
            data = read(family)
            if family == 'arc_blend_command':
                row = next(r for r in data['results'] if r['id'] == 'arc_radius_difference')
                row['value']['input_trace'][0]['errors'] = []
            return data
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'command diagnostic changed'):
                evidence.collect(PROOF)

    def test_native_success_requires_acknowledged_physical_picks(self):
        def changed(family):
            data = read(family)
            if family == 'arc_blend_command':
                data['results'][0]['value']['input_trace'][0]['clicks'][0]['click_acknowledged'] = False
            return data
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'unacknowledged native pick'):
                evidence.collect(PROOF)

    def test_reconstruction_rejects_missing_replay_and_invalid_geometry_error(self):
        path = ROOT / 'docs/arc-blend-validation.json'
        if not path.exists():
            self.skipTest('record follows implementation commit')
        value = json.loads(path.read_bytes())
        revision = value['implementation_revision']
        self.assertEqual(value, report.build(PROOF, revision))
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'arc-ui-portable.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i, line in enumerate(lines) if line.startswith('ARC_BLEND_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index] + lines[index+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'):
                report.build(proof, revision)
            for error in [1.1e-6, float('nan')]:
                row = json.loads(lines[index].split(' ', 1)[1])
                row['maximum_coefficient_and_sample_error'] = error
                changed = lines.copy()
                changed[index] = 'ARC_BLEND_UI ' + json.dumps(row) + '\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'geometry gate failed'):
                    report.build(proof, revision)

if __name__ == '__main__':
    unittest.main()
