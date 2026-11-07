"""Reject incomplete captures, comparison gaps and nonfinite geometry claims."""
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import fillet_closed_evidence as evidence, report_fillet_closed as report
from .check_fillet_closed import ROOT, PROOF


class FilletClosedTests(unittest.TestCase):
    def test_complete_capture_and_failed_attempts_have_separate_counts(self):
        rows, captures, counts, paths = evidence.collect(PROOF)
        self.assertEqual((len(rows), len(captures)), (16, 2))
        self.assertEqual(counts, dict(observations=16, successful=15, cancelled=1, physical_picks=32, point_tangent_stations=2442))
        self.assertGreater(len(paths), 20)

    def test_failed_capture_cannot_contribute_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof)
            p = proof / 'failed-spatial.json'; v = json.loads(p.read_bytes()); v['native_observations'] = 6; p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError, 'failed capture'): evidence.collect(proof)

    def test_frozen_recipe_closure_covers_both_standalone_imports(self):
        for family in evidence.FAMILIES:
            v = json.loads(gzip.decompress((evidence.HERE / ('observations/' + family + '.json.gz')).read_bytes()))
            self.assertTrue({'fillet_closed_cases.py', 'fillet_command.py', 'fillet_command_probe.py'}.issubset(v['provenance']['sources']))

    def test_missing_and_nonfinite_comparisons_fail_the_gate(self):
        if not (PROOF / 'closed-command-portable.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof); p = proof / 'closed-command-portable.log.gz'
            lines = gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True); i = next(i for i, line in enumerate(lines) if line.startswith('FILLET_CLOSED_COMMAND ')); row = json.loads(lines[i].split(' ', 1)[1])
            for value in [float('nan'), 1.1e-6]:
                changed = lines.copy(); changed[i] = 'FILLET_CLOSED_COMMAND ' + json.dumps(dict(row, maximum_geometry_error=value)) + '\n'; p.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'geometry gate'): report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i] + lines[i+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        p = ROOT / 'docs/fillet-closed-validation.json'
        if not p.exists(): self.skipTest('record follows implementation commit')
        value = json.loads(p.read_bytes()); self.assertEqual(value, report.build(PROOF, value['implementation_revision']))

if __name__ == '__main__': unittest.main()
