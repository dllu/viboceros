"""Keep incomplete averaging modes and failed launches outside qualification."""
import copy,json,shutil,tempfile,unittest
from pathlib import Path
from .surface_average_evidence import PROOF,collect,PENDING_COMMAND

class SurfaceAverageEvidenceTests(unittest.TestCase):
    def test_complete_scope_and_explicit_pending_modes(self):
        api,pending,command,_=collect()
        self.assertEqual((len(api),len(pending),len(command)),(54,12,8))
        self.assertTrue(all(v['prescribed_case']['continuity']in ['Position','Tangency'] and not v['prescribed_case']['refine']for v in api.values()))
        self.assertEqual(set(command)&PENDING_COMMAND,{'match_average_curvature_pending'})
        self.assertEqual({v['prescribed_case']['reverse_target']for v in api.values()},{False,True})
    def test_receipts_cannot_hide_missing_failed_or_unowned_launches(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'captures.json';rows=json.loads(path.read_bytes())
            for changed in [rows[:-1],rows+rows[-1:]]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):collect(proof)
            for key,bad in [('exit_status',None),('command',['desktop-launcher']),('observation_sha256','0'*64)]:
                changed=copy.deepcopy(rows);changed[0][key]=bad;path.write_text(json.dumps(changed))
                with self.subTest(key=key),self.assertRaises(ValueError):collect(proof)
    def test_failed_dispatch_attempt_remains_accounted_for(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'attempts.json';rows=json.loads(path.read_bytes())
            self.assertEqual(rows[0]['exit_status'],1)
            for changed in [[],rows+rows]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):collect(proof)

if __name__=='__main__':unittest.main()
