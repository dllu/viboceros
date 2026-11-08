"""Reject incomplete captures, comparison gaps and nonfinite geometry claims."""
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import chamfer_closed_evidence as evidence, report_chamfer_closed as report
from .check_chamfer_closed import ROOT, PROOF


class ChamferClosedTests(unittest.TestCase):
    def test_complete_capture_preserves_all_command_outcomes(self):
        rows, captures, counts, paths = evidence.collect(PROOF)
        self.assertEqual((len(rows), len(captures)), (33, 3))
        self.assertEqual(counts, dict(observations=33, successful=30, cancelled=1, failed=2, changed=29, images=0, physical_picks=63, point_tangent_stations=4950))
        self.assertGreater(len(paths), 20)

    def test_failed_capture_cannot_contribute_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof)
            p = proof / 'captures.json'; v = json.loads(p.read_bytes()); v[0]['exit_status'] = 1; p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError, 'headless capture'): evidence.collect(proof)

    def test_frozen_recipe_closure_covers_both_standalone_imports(self):
        for family in evidence.FAMILIES:
            v = json.loads(gzip.decompress((evidence.HERE / ('observations/' + family + '.json.gz')).read_bytes()))
            self.assertTrue({'chamfer_closed_cases.py', 'chamfer_command.py', 'chamfer_command_probe.py'}.issubset(v['provenance']['sources']))

    def test_missing_and_nonfinite_comparisons_fail_the_gate(self):
        if not (PROOF / 'closed-command-portable.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof); p = proof / 'closed-command-portable.log.gz'
            lines = gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True); i = next(i for i, line in enumerate(lines) if line.startswith('CHAMFER_CLOSED_COMMAND ')); row = json.loads(lines[i].split(' ', 1)[1])
            for value in [float('nan'), 1.1e-6]:
                changed = lines.copy(); changed[i] = 'CHAMFER_CLOSED_COMMAND ' + json.dumps(dict(row, maximum_geometry_error=value)) + '\n'; p.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'geometry gate'): report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i] + lines[i+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        p = ROOT / 'docs/chamfer-closed-validation.json'
        if not p.exists(): self.skipTest('record follows implementation commit')
        value = json.loads(p.read_bytes()); self.assertEqual(value, report.build(PROOF, value['implementation_revision']))

if __name__ == '__main__': unittest.main()
