"""Reproduce the menu routing qualification and reject incomplete checks."""
import hashlib,json,shutil,subprocess,tempfile,unittest
from pathlib import Path
from .report_match_surface_menus import ROOT,PROOF,build

class MatchSurfaceMenuRecordTests(unittest.TestCase):
    def record(self):return json.loads((ROOT/'docs/match-surface-menu-validation.json').read_bytes())
    def test_record_reproduces_and_hashes_match(self):
        value=self.record();self.assertEqual(value,build(PROOF,value['implementation_revision']))
        for p,digest in value['evidence_sha256'].items():self.assertEqual(hashlib.sha256((ROOT/p).read_bytes()).hexdigest(),digest,p)
        for p,digest in value['implementation_git_sha256'].items():self.assertEqual(hashlib.sha256(subprocess.check_output(['git','cat-file','blob',p],cwd=ROOT)).hexdigest(),digest,p)
    def test_missing_duplicate_and_failed_checks_are_rejected(self):
        value=self.record()
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'checks.json';rows=json.loads(path.read_bytes());failed=json.loads(json.dumps(rows));failed[0]['status']=1
            for changed in [rows[:-1],rows+rows[-1:],failed]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof,value['implementation_revision'])

if __name__=='__main__':unittest.main()
