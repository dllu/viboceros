"""Reject incomplete, unowned or incorrectly scoped averaged G2 captures."""
import copy,json,shutil,tempfile,unittest
from pathlib import Path
from .surface_average_curvature_evidence import PROOF,collect

class SurfaceAverageCurvatureTests(unittest.TestCase):
    def test_scope_retains_refinement_as_pending(self):
        averages,curvature,pending,commands,_=collect()
        self.assertEqual((len(averages),len(curvature),len(pending),len(commands)),(54,65,2,8))
        self.assertTrue(all(v['prescribed_case']['continuity']=='Curvature'and not v['prescribed_case']['refine']for v in curvature.values()))
        self.assertEqual(pending,{'average_h_position_refine','average_h_tangency_refine'})
    def test_headless_receipts_are_complete_and_owned(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'captures.json';rows=json.loads(path.read_bytes())
            variants=[rows[:-1],rows+rows[-1:]]
            for key,bad in [('exit_status',None),('command',['desktop-launcher']),('observation_sha256','0'*64)]:
                changed=copy.deepcopy(rows);changed[0][key]=bad;variants.append(changed)
            for changed in variants:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):collect(proof)

if __name__=='__main__':unittest.main()
