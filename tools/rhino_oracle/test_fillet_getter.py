"""Guard immutable capture inputs, diagnostics and finite Native comparison gates."""
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import fillet_command as recipes, fillet_getter_evidence as evidence, report_fillet_getter as report
from .check_fillet_getter import ROOT, PROOF


class FilletGetterTests(unittest.TestCase):
    def test_complete_workflows_and_failed_attempts_have_separate_coverage(self):
        rows, diagnostics, captures, counts, paths = evidence.collect(PROOF)
        self.assertEqual((len(rows), len(diagnostics), len(captures)), (25, 4, 4))
        self.assertEqual((counts['successful'], counts['changed'], counts['cancelled'], counts['picks'], counts['samples'], counts['images']), (16, 14, 9, 41, 3300, 8))
        self.assertGreater(len(paths), 30)

    def test_recipe_validator_rejects_mutable_or_arbitrary_inputs(self):
        recipes.validate(dict(op='fillet_command', id='numeric_then_enter', case='numeric_then_enter'))
        for op in [dict(op='fillet_command', id='other', case='numeric_then_enter'), dict(op='fillet_command', id='numeric_then_enter', case='numeric_then_enter', radius=9.), dict(op='fillet_command', id='bad', case='bad'), dict(op='fillet_command', id='bad', case=[])]:
            with self.assertRaises(ValueError): recipes.validate(op)

    def test_failed_capture_cannot_publish_matched_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof)
            p = proof / 'capture-attempts.json'; rows = json.loads(p.read_bytes()); rows[1]['native_observations'] = 9; p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError, 'failed capture receipt'): evidence.collect(proof)

    def test_missing_nonfinite_or_out_of_gate_results_are_rejected(self):
        if not (PROOF / 'command-portable.log.gz').exists(): self.skipTest('command receipts follow implementation checks')
        with tempfile.TemporaryDirectory(dir=ROOT / 'docs') as directory:
            proof = Path(directory) / 'proof'; shutil.copytree(PROOF, proof); p = proof / 'command-portable.log.gz'
            lines = gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True); i = next(i for i, line in enumerate(lines) if line.startswith('FILLET_GETTER_COMMAND ')); row = json.loads(lines[i].split(' ', 1)[1])
            for field, error, message in [('maximum_geometry_error', 1.1e-6, 'geometry gate'), ('maximum_geometry_error', float('nan'), 'geometry gate'), ('archives', 0, 'archive gate')]:
                changed = lines.copy(); changed[i] = 'FILLET_GETTER_COMMAND ' + json.dumps(dict(row, **{field: error})) + '\n'; p.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, message): report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i] + lines[i+1:]).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        p = ROOT / 'docs/fillet-getter-validation.json'
        if not p.exists(): self.skipTest('record follows implementation commit')
        value = json.loads(p.read_bytes()); self.assertEqual(value, report.build(PROOF, value['implementation_revision']))

if __name__ == '__main__': unittest.main()
