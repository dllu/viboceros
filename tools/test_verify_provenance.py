import hashlib
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from .verify_provenance import reconstruct_measurement, verify, verify_measurement


class ProvenanceTests(unittest.TestCase):
    def git(self, root, *args):
        return subprocess.check_output(['git', *args], cwd=root).decode().strip()

    def commit(self, root, *paths):
        self.git(root, 'add', '--', *paths)
        self.git(root, '-c', 'user.name=Test', '-c', 'user.email=test@example.org',
                 '-c', 'commit.gpgsign=false', 'commit', '-qm', 'Measurement')
        return self.git(root, 'rev-parse', 'HEAD')

    def measurement(self, root):
        self.git(root, 'init', '-q')
        source = root / 'source.rs'
        source.write_bytes(b'original implementation\n')
        original = dict(sha256={'source.rs': hashlib.sha256(source.read_bytes()).hexdigest()},
                        cases=[dict(id='curve', error=0.1)], full_parity=False)
        encoded = (json.dumps(original, indent=2) + '\n').encode()
        (root / 'report.json').write_bytes(encoded)
        revision = self.commit(root, 'source.rs', 'report.json')
        return dict(original, measurement_revision=revision, measurement_record='report.json',
                    measurement_record_sha256=hashlib.sha256(encoded).hexdigest(),
                    measurement_scope='Historical results; current implementation not requalified.')

    def test_historical_payload_and_complete_source_inventory_cannot_be_rewritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = self.measurement(root)
            (root / 'source.rs').unlink()
            self.assertEqual(verify(record, root), 2)
            for update in [dict(sha256={}), dict(sha256={'source.rs': '0' * 64}),
                           dict(cases=[]), dict(full_parity=True), dict(full_parity=0),
                           dict(measurement_record_sha256='0' * 64),
                           dict(measurement_revision='HEAD'), dict(measurement_revision='f' * 40),
                           dict(measurement_revision=self.git(root, 'rev-parse', 'HEAD^{tree}'))]:
                with self.subTest(update=update), self.assertRaises(ValueError):
                    verify(dict(record, **update), root)

    def test_reconstruction_preserves_scope_and_exposes_changed_measurements(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = self.measurement(root)
            (root / 'source.rs').write_bytes(b'changed implementation\n')
            def summarize(text):
                rows = json.loads(text)
                if len(rows) != 1 or rows[0]['id'] != 'curve':
                    raise ValueError('incomplete comparisons')
                return dict(cases=rows, full_parity=False, sha256={
                    'source.rs': hashlib.sha256((root / 'source.rs').read_bytes()).hexdigest()})
            text = json.dumps(record['cases'])
            self.assertNotEqual(summarize(text)['sha256'], record['sha256'])
            self.assertEqual(reconstruct_measurement(record, summarize, text, root=root), record)
            with self.assertRaisesRegex(ValueError, 'incomplete comparisons'):
                reconstruct_measurement(record, summarize, '[]', root=root)
            rows = copy.deepcopy(record['cases'])
            rows[0]['error'] = 0.15
            with self.assertRaisesRegex(ValueError, 'reconstructed measurements differ'):
                reconstruct_measurement(record, summarize, json.dumps(rows), root=root)
            fresh = summarize(text)
            self.assertEqual(reconstruct_measurement(fresh, summarize, text, root=root), fresh)
            self.assertEqual(verify_measurement(fresh, root, expand_sources=lambda paths: paths), 1)
            with self.assertRaisesRegex(ValueError, 'inventory is incomplete'):
                verify_measurement(fresh, root, expand_sources=lambda paths: paths + ['missing.rs'])
            with self.assertRaisesRegex(ValueError, 'current measurements or source bindings differ'):
                reconstruct_measurement(dict(fresh, sha256={}), summarize, text, root=root)
            with self.assertRaisesRegex(ValueError, 'committed original'):
                verify_measurement({key: value for key, value in record.items()
                                    if key != 'measurement_record'}, root)

    def test_parent_revision_selects_historical_submodule_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            child = root / 'vendor/kernel'
            child.mkdir(parents=True)
            for folder in [root, child]:
                self.git(folder, 'init', '-q')
            source = child / 'kernel.rs'
            source.write_bytes(b'original kernel\n')
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            first = self.commit(child, 'kernel.rs')
            self.git(root, 'update-index', '--add', '--cacheinfo', '160000', first, 'vendor/kernel')
            (root / 'marker').write_text('original parent\n')
            revision = self.commit(root, 'marker')
            source.write_bytes(b'new kernel\n')
            second = self.commit(child, 'kernel.rs')
            self.git(root, 'update-index', '--cacheinfo', '160000', second, 'vendor/kernel')
            (root / 'marker').write_text('new parent\n')
            self.commit(root, 'marker')
            record = dict(measurement_revision=revision, sha256={'vendor/kernel/kernel.rs': digest})
            self.assertEqual(verify(record, root), 1)
            source.unlink()
            self.assertEqual(verify(record, root), 1)
            with self.assertRaises(ValueError):
                verify(dict(record, sha256={'vendor/kernel/kernel.rs': '0' * 64}), root)

    def test_deleted_source_remains_verifiable_through_git_history(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.rs"
            source.write_bytes(b"historical source\n")
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            subprocess.run(["git", "init", "-q", directory], check=True)
            subprocess.run(["git", "add", "source.rs"], cwd=root, check=True)
            subprocess.run([
                "git", "-c", "user.name=Test", "-c", "user.email=test@example.org",
                "-c", "commit.gpgsign=false", "commit", "-qm", "Source",
            ], cwd=root, check=True)
            revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
            key = f"{revision}:source.rs"
            source.unlink()
            self.assertEqual(verify({"source_git_sha256": {key: digest}}, root), 1)
            for reference, expected in [
                (key, "0" * 64),
                (f"{revision}:missing.rs", digest),
                ("HEAD:source.rs", digest),
                (key, "invalid"),
            ]:
                with self.subTest(reference=reference, expected=expected), self.assertRaises(ValueError):
                    verify({"source_git_sha256": {reference: expected}}, root)

    def test_local_artifacts_are_still_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "capture.json"
            artifact.write_bytes(b"{}\n")
            digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
            record = {"sha256": {"capture.json": digest}, "scope": "historical"}
            self.assertEqual(verify(record, root), 1)
            artifact.write_bytes(b"[]\n")
            with self.assertRaises(ValueError):
                verify(record, root)


if __name__ == "__main__":
    unittest.main()
