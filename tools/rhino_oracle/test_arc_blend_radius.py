"""Strict target classifications, independent arc data and replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock
from . import arc_blend_radius_evidence as evidence, report_arc_blend_radius as report
from .check_arc_blend_radius import ROOT, PROOF
from .client import OracleClient, OracleProtocolError

class ArcBlendRadiusTests(unittest.TestCase):
    def test_capture_receipts_frozen_sources_and_independent_samples(self):
        matched, diagnostics, captures, physical, paths = evidence.collect(PROOF)
        self.assertEqual((len(matched), len(diagnostics), len(captures)), (18, 23, 3))
        self.assertEqual(Counter(diagnostics.values()), {
            'general tangent radius solver remains unsupported': 21,
            'target inherited from a previous command remains unsupported': 2})
        self.assertEqual(sum('Unable to achieve' in v['history'] for v in matched.values()), 2)
        self.assertGreaterEqual(physical, 36)
        self.assertGreater(len(paths), 30)

    def test_new_requests_remain_private_fixed_bounded_and_unique(self):
        with mock.patch.dict('os.environ', {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), mock.patch('subprocess.Popen') as launch:
            for names in evidence.FAMILIES.values():
                request = evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):
                    OracleClient().run_rhino(request)
                for iterations in [True, 1.0, 0, 2]:
                    changed = copy.deepcopy(request)
                    changed['iterations'] = iterations
                    with self.assertRaises(OracleProtocolError):
                        OracleClient(settings_scheme='VibocerosOracleArcRadiusInvalid').run_rhino(changed)
                for multiplier in [2, 65]:
                    changed = copy.deepcopy(request)
                    changed['operations'] *= multiplier
                    with self.assertRaises(OracleProtocolError):
                        OracleClient(settings_scheme='VibocerosOracleArcRadiusInvalid').run_rhino(changed)
                for mutation in [dict(case=None), dict(case=[]), dict(case='foreign'), dict(id='foreign'), dict(script='ArcBlend')]:
                    with self.assertRaises(ValueError):
                        evidence.recipes.validate(dict(request['operations'][0], **mutation))
            launch.assert_not_called()

    def test_missing_or_nonfinite_public_samples_fail_evidence_audit(self):
        read = evidence.read
        for mutation in ['missing', 'nan']:
            def changed(family):
                value = read(family)
                if family == 'arc_blend_radius_targets':
                    output = next(o for o in value['results'][0]['value']['after'] if 'arc_segments' in o)
                    if mutation == 'missing':
                        output['arc_segments'][0]['samples'].pop()
                    else:
                        output['arc_segments'][0]['samples'][0]['point'][0] = float('nan')
                    value['results'][0]['value']['redo'] = copy.deepcopy(value['results'][0]['value']['after'])
                return value
            with mock.patch.object(evidence, 'read', side_effect=changed):
                with self.assertRaisesRegex(ValueError, 'independent arc'):
                    evidence.collect(PROOF)

    def test_incomplete_native_input_cannot_pass_as_a_geometry_match(self):
        read = evidence.read
        def changed(family):
            value = read(family)
            value['results'][0]['value']['input_trace'][0]['completed'] -= 1
            return value
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'incomplete native workflow'):
                evidence.collect(PROOF)

    def test_reconstruction_requires_all_geometry_and_independent_sample_replays(self):
        path = ROOT / 'docs/arc-blend-radius-validation.json'
        if not path.exists():
            self.skipTest('record follows implementation commit')
        value = json.loads(path.read_bytes())
        revision = value['implementation_revision']
        self.assertEqual(value, report.build(PROOF, revision))
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'radius-ui-portable.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i, line in enumerate(lines) if line.startswith('ARC_BLEND_RADIUS_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index] + lines[index+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'):
                report.build(proof, revision)
            for field in ['maximum_coefficient_and_sample_error', 'maximum_native_arc_sample_error']:
                for error in [1.1e-6, float('nan')]:
                    row = json.loads(lines[index].split(' ', 1)[1])
                    row[field] = error
                    changed = lines.copy()
                    changed[index] = 'ARC_BLEND_RADIUS_UI ' + json.dumps(row) + '\n'
                    path.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                    with self.assertRaisesRegex(ValueError, 'geometry gate failed'):
                        report.build(proof, revision)

if __name__ == '__main__':
    unittest.main()
