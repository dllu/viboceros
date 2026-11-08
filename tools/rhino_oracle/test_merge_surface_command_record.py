"""Reproduce the pinned command record and reject incomplete check receipts."""
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
import unittest
from .report_merge_surface_command import ROOT, PROOF, build


class MergeSurfaceRecordTests(unittest.TestCase):
    def record(self):
        return json.loads((ROOT / 'docs/merge-srf-command-validation.json').read_bytes())

    def test_pinned_record_reproduces(self):
        record = self.record()
        self.assertEqual(record, build(PROOF, record['implementation_revision']))
        self.assertFalse(record['full_native_parity'])
        self.assertFalse(record['continuous_surface_distance_qualified'])
        for path, digest in record['evidence_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
        for source, digest in record['implementation_git_sha256'].items():
            blob = subprocess.check_output(['git', 'cat-file', 'blob', source], cwd=ROOT)
            self.assertEqual(hashlib.sha256(blob).hexdigest(), digest, source)

    def test_failed_or_missing_checks_do_not_qualify(self):
        record = self.record()
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'checks.json'
            checks = json.loads(path.read_bytes())
            failed = json.loads(json.dumps(checks)); failed[0]['status'] = 1
            for altered in [checks[:-1], checks + checks[-1:], failed]:
                path.write_text(json.dumps(altered))
                with self.assertRaises(ValueError): build(proof, record['implementation_revision'])


if __name__ == '__main__': unittest.main()
