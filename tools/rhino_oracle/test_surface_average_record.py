"""Reproduce the bounded averaging record and reject lost outputs or workflows."""
import gzip,hashlib,json,shutil,subprocess,tempfile,unittest
from pathlib import Path
from .report_surface_average import ROOT,PROOF,build,comparisons

class SurfaceAverageRecordTests(unittest.TestCase):
    def record(self):return json.loads((ROOT/'docs/surface-average-validation.json').read_bytes())
    def test_record_reproduces_and_hashes_match(self):
        value=self.record();self.assertEqual(value,build(PROOF,value['implementation_revision']))
        self.assertFalse(value['curvature_averaging_qualified']);self.assertFalse(value['averaged_refinement_qualified'])
        for p,digest in value['evidence_sha256'].items():self.assertEqual(hashlib.sha256((ROOT/p).read_bytes()).hexdigest(),digest,p)
        for p,digest in value['implementation_git_sha256'].items():self.assertEqual(hashlib.sha256(subprocess.check_output(['git','cat-file','blob',p],cwd=ROOT)).hexdigest(),digest,p)
    def test_missing_duplicate_and_failed_checks_are_rejected(self):
        value=self.record()
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'checks.json';rows=json.loads(path.read_bytes())
            failed=json.loads(json.dumps(rows));failed[0]['status']=1
            for changed in [rows[:-1],rows+rows[-1:],failed]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof,value['implementation_revision'])
    def test_pairs_cannot_hide_a_missing_output_or_failed_geometry_comparison(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'average-portable.log.gz'
            original=gzip.decompress(path.read_bytes()).decode().splitlines();index=next(i for i,s in enumerate(original)if s.startswith('AVERAGE_SURFACE '))
            row=json.loads(original[index].split(' ',1)[1]);variants=[original[:index]+original[index+1:],original+[original[index]]]
            for key,bad in [('outputs',1),('archives',True),('maximum_geometry_error',1e-3),('maximum_geometry_error',float('nan'))]:
                changed=dict(row);changed[key]=bad;lines=original[:];lines[index]='AVERAGE_SURFACE '+json.dumps(changed);variants.append(lines)
            for lines in variants:
                path.write_bytes(gzip.compress('\n'.join(lines).encode(),mtime=0))
                with self.assertRaises(ValueError):comparisons(proof)

if __name__=='__main__':unittest.main()
