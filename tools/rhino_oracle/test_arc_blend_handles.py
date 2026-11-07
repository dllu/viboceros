"""Retained handle inputs, strict classifications and reconstruction failures."""
import copy
import gzip
import hashlib
import json
import shutil
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock
from . import arc_blend_handle_evidence as evidence, report_arc_blend_handles as report
from .arc_blend_handle_evidence import read
from .check_arc_blend_handles import ROOT, PROOF
from .client import OracleClient, OracleProtocolError

class ArcBlendHandleTests(unittest.TestCase):
    def test_native_recipes_receipts_snapshots_and_all_diagnostics_are_retained(self):
        matched, diagnostics, captures, picks, queries, paths = evidence.collect(PROOF)
        self.assertEqual((len(matched), len(diagnostics), len(captures), picks, len(queries), sum(queries.values())), (26, 18, 5, 83, 4, 2))
        self.assertEqual(Counter(diagnostics.values()), {
            'incomplete getter driver': 4, 'automatic radius solving remains unsupported': 9,
            'native rejected the input': 3, 'intended adjustment handle was not selected': 2,
        })
        self.assertGreater(len(paths), 30)
        for family in evidence.FAMILIES:
            for digest in read(family)['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((evidence.HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())).hexdigest(), digest)

    def test_new_recipes_require_private_unique_bounded_fixed_inputs(self):
        with mock.patch.dict('os.environ', {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), mock.patch('subprocess.Popen') as launch:
            for names in evidence.FAMILIES.values():
                request = evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):
                    OracleClient().run_rhino(request)
                for iterations in [True, 1.0, 0, 2]:
                    changed = copy.deepcopy(request)
                    changed['iterations'] = iterations
                    with self.assertRaises(OracleProtocolError):
                        OracleClient(settings_scheme='VibocerosOracleArcHandlesInvalid').run_rhino(changed)
                for multiplier in [2, 65]:
                    changed = copy.deepcopy(request)
                    changed['operations'] *= multiplier
                    with self.assertRaises(OracleProtocolError):
                        OracleClient(settings_scheme='VibocerosOracleArcHandlesInvalid').run_rhino(changed)
                for mutation in [dict(case=None), dict(case=[]), dict(case='foreign'), dict(id='foreign'), dict(script='ArcBlend')]:
                    operation = dict(request['operations'][0], **mutation)
                    with self.assertRaises(ValueError):
                        evidence.recipes.validate(operation)
            launch.assert_not_called()

    def test_public_projection_matrices_must_be_finite(self):
        def changed(family):
            value = read(family)
            if family == 'arc_blend_chord_handles':
                value['results'][0]['value']['input_trace'][0]['clicks'][-1]['pick_transform'][0][0] = float('nan')
            return value
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'invalid public projection matrix'):
                evidence.collect(PROOF)

    def test_partial_driver_inputs_cannot_be_promoted_to_matching_workflows(self):
        def changed(family):
            value = read(family)
            if family == 'arc_blend_handles':
                value['results'][2]['value']['input_trace'][0]['errors'] = []
            return value
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'incomplete native workflow'):
                evidence.collect(PROOF)

    def test_native_source_identity_must_survive_trimming(self):
        def changed(family):
            value = read(family)
            if family == 'arc_blend_handle_holdouts':
                v = next(r['value'] for r in value['results'] if r['id'] == 'arc_handle_trim')
                next(o for o in v['after'] if o['attributes']['name'] == 'guide0')['guide'] = False
                v['redo'] = copy.deepcopy(v['after'])
            return value
        with mock.patch.object(evidence, 'read', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'native source identity was not retained'):
                evidence.collect(PROOF)

    def test_reconstruction_rejects_missing_geometry_and_cursor_replays(self):
        path = ROOT / 'docs/arc-blend-handles-validation.json'
        if not path.exists():
            self.skipTest('record follows implementation commit')
        value = json.loads(path.read_bytes())
        revision = value['implementation_revision']
        self.assertEqual(value, report.build(PROOF, revision))
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'handles-ui-portable.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i, line in enumerate(lines) if line.startswith('ARC_BLEND_HANDLE_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index] + lines[index+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'):
                report.build(proof, revision)
            for error in [1.1e-6, float('nan')]:
                row = json.loads(lines[index].split(' ', 1)[1])
                row['maximum_coefficient_and_sample_error'] = error
                changed = lines.copy()
                changed[index] = 'ARC_BLEND_HANDLE_UI ' + json.dumps(row) + '\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'geometry gate failed'):
                    report.build(proof, revision)
            path.write_bytes(gzip.compress(''.join(lines).encode(), mtime=0))
            index = next(i for i, line in enumerate(lines) if line.startswith('ARC_BLEND_HANDLE_VIEW_LINE '))
            path.write_bytes(gzip.compress(''.join(lines[:index] + lines[index+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete viewing-line queries'):
                report.build(proof, revision)

if __name__ == '__main__':
    unittest.main()
