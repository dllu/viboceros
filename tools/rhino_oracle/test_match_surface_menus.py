"""Corrected owned enum capture and strict validation of its physical-input receipt."""
import copy,json,shutil,tempfile,unittest
from pathlib import Path
from .match_surface_menu_evidence import PROOF,collect

class MatchSurfaceMenuTests(unittest.TestCase):
    def test_complete_menu_capture_and_frozen_producers(self):
        value,paths=collect();self.assertTrue(value['success']);self.assertGreater(len(paths),20)
    def test_missing_status_and_unowned_launch_do_not_qualify(self):
        with tempfile.TemporaryDirectory() as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'capture.json';receipt=json.loads(path.read_bytes())
            for key,bad in [('command',['desktop-launcher']),('exit_status',None),('observation_sha256','0'*64),('qualified_rows',[])]:
                changed=copy.deepcopy(receipt);changed[key]=bad;path.write_text(json.dumps(changed))
                with self.subTest(key=key),self.assertRaises(ValueError):collect(proof)

if __name__=='__main__':unittest.main()
