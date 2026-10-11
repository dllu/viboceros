import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

from .verify_provenance import verify


class ProvenanceTests(unittest.TestCase):
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
