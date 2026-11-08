"""Reproduce MatchSrf command qualification with source and evidence hashes."""
import hashlib,json,shutil,subprocess,tempfile,unittest
from pathlib import Path
from .report_match_surface_command import ROOT,PROOF,build


class MatchSurfaceCommandRecordTests(unittest.TestCase):
    def record(self):return json.loads((ROOT/'docs/match-surface-command-validation.json').read_bytes())
    def test_record_reproduces_and_hashes_match(self):
        value=self.record();self.assertEqual(value,build(PROOF,value['implementation_revision']))
        self.assertFalse(value['full_native_parity']);self.assertEqual(value['ui_replays_per_build'],21)
        for path,digest in value['evidence_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
        for source,digest in value['implementation_git_sha256'].items():
            blob=subprocess.check_output(['git','cat-file','blob',source],cwd=ROOT)
            self.assertEqual(hashlib.sha256(blob).hexdigest(),digest,source)
    def test_missing_duplicate_or_failed_checks_do_not_qualify(self):
        value=self.record()
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'checks.json';rows=json.loads(path.read_bytes())
            failed=json.loads(json.dumps(rows));failed[0]['status']=1
            for changed in [rows[:-1],rows+rows[-1:],failed]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof,value['implementation_revision'])


if __name__=='__main__':unittest.main()
