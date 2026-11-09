"""Reproduce the G2 record and reject missing pairs, workflows or error gates."""
import gzip,hashlib,json,shutil,subprocess,tempfile,unittest
from pathlib import Path
from .report_surface_average_curvature import ROOT,PROOF,build,comparisons

class SurfaceAverageCurvatureRecordTests(unittest.TestCase):
    def record(self):return json.loads((ROOT/'docs/surface-average-curvature-validation.json').read_bytes())
    def test_record_and_hashes_reproduce(self):
        value=self.record();self.assertEqual(value,build(PROOF,value['implementation_revision']))
        self.assertTrue(value['curvature_averaging_qualified']);self.assertFalse(value['averaged_refinement_qualified'])
        for p,digest in value['evidence_sha256'].items():self.assertEqual(hashlib.sha256((ROOT/p).read_bytes()).hexdigest(),digest,p)
        for p,digest in value['implementation_git_sha256'].items():self.assertEqual(hashlib.sha256(subprocess.check_output(['git','cat-file','blob',p],cwd=ROOT)).hexdigest(),digest,p)
    def test_checks_cannot_be_missing_duplicated_or_failed(self):
        value=self.record()
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'checks.json';rows=json.loads(path.read_bytes())
            failed=json.loads(json.dumps(rows));failed[0]['status']=1
            for changed in [rows[:-1],rows+rows[-1:],failed]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof,value['implementation_revision'])
    def test_curvature_pairs_and_workflows_are_required(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            for file,prefix in [('average-portable','AVERAGE_CURVATURE '),('command-native','AVERAGE_SURFACE_COMMAND '),('ui-native','AVERAGE_SURFACE_UI ')]:
                path=proof/(file+'.log.gz');raw=path.read_bytes();lines=gzip.decompress(raw).decode().splitlines();i=next(i for i,s in enumerate(lines)if s.startswith(prefix));row=json.loads(lines[i][len(prefix):])
                variants=[lines[:i]+lines[i+1:],lines+[lines[i]]]
                for key,bad in [('archives',True),('maximum_geometry_error',1e-3),('maximum_geometry_error',float('nan'))]:
                    changed=dict(row);changed[key]=bad;edited=lines[:];edited[i]=prefix+json.dumps(changed);variants.append(edited)
                if file=='average-portable':
                    changed=dict(row,outputs=1);edited=lines[:];edited[i]=prefix+json.dumps(changed);variants.append(edited)
                for edited in variants:
                    path.write_bytes(gzip.compress('\n'.join(edited).encode(),mtime=0))
                    with self.assertRaises(ValueError):comparisons(proof)
                path.write_bytes(raw)

if __name__=='__main__':unittest.main()
