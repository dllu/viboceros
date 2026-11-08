"""Owned tolerance discovery stays distinct from verified fitting behavior."""
import copy,json,shutil,tempfile,unittest
from pathlib import Path
from .surface_match_tolerance_evidence import ROOT,PROOF,collect,summary

class SurfaceMatchToleranceEvidenceTests(unittest.TestCase):
    def test_owned_inventory_and_known_instrumentation_defect(self):
        rows,captures,_=collect();self.assertEqual((len(rows),len(captures)),(78,14))
        self.assertFalse(summary()['custom_fitting_qualified'])
        for key in ['match_local_closest_audit_rational_position','match_local_closest_audit_rational_curvature','match_local_closest_audit_polynomial_position']:
            self.assertTrue(rows[key]['prescribed_case']['local_closest_audit'])
            self.assertTrue(all('local_closest' not in r for r in rows[key]['audit']['samples']))
        self.assertTrue(all('local_closest' in r for r in rows['match_local_query_audit_rational_position']['audit']['samples']))
    def test_curvature_sweep_is_an_observation_not_a_continuity_certificate(self):
        rows,_,_=collect();values=[rows['match_tol_sdk_curvature_'+str(x)]for x in [1e-5,.05,1.,100.]]
        self.assertTrue(all(v['output']==values[0]['output']for v in values[1:]))
        self.assertEqual([v['audit']['settings']['curvature']for v in values],[1e-5,.05,1.,100.])
    def test_missing_status_and_duplicate_launches_cannot_be_hidden(self):
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'captures.json';receipts=json.loads(path.read_bytes())
            for changed in [receipts[:-1],receipts+receipts[-1:]]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):collect(proof)
            for key,bad in [('command',['desktop-launcher']),('exit_status',None),('observation_sha256','0'*64)]:
                changed=copy.deepcopy(receipts);changed[0][key]=bad;path.write_text(json.dumps(changed))
                with self.subTest(key=key),self.assertRaises(ValueError):collect(proof)
    def test_catalog_reproduces(self):
        self.assertEqual(json.loads((ROOT/'docs/surface-match-tolerance-investigation.json').read_bytes()),summary())

if __name__=='__main__':unittest.main()
