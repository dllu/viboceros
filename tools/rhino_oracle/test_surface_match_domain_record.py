"""Domain regressions cannot stand in for native tolerance-fitting parity."""
import hashlib,json,gzip,shutil,subprocess,tempfile,unittest
from pathlib import Path
from .report_surface_match_domain import ROOT,PROOF,build,domains

class SurfaceMatchDomainRecordTests(unittest.TestCase):
    def record(self):return json.loads((ROOT/'docs/surface-match-domain-validation.json').read_bytes())
    def test_record_reproduces_and_hashes_match(self):
        value=self.record();self.assertEqual(value,build(PROOF,value['implementation_revision']))
        self.assertFalse(value['custom_fitting_qualified'])
        for p,digest in value['evidence_sha256'].items():self.assertEqual(hashlib.sha256((ROOT/p).read_bytes()).hexdigest(),digest,p)
        for p,digest in value['implementation_git_sha256'].items():self.assertEqual(hashlib.sha256(subprocess.check_output(['git','cat-file','blob',p],cwd=ROOT)).hexdigest(),digest,p)
    def test_incomplete_checks_are_rejected(self):
        value=self.record()
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'checks.json';rows=json.loads(path.read_bytes())
            failed=json.loads(json.dumps(rows));failed[0]['status']=1
            for changed in [rows[:-1],rows+rows[-1:],failed]:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):build(proof,value['implementation_revision'])
    def test_domain_inventory_and_error_gates(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)
            for mode in ['portable','native']:shutil.copyfile(PROOF/('domain-'+mode+'.log.gz'),proof/('domain-'+mode+'.log.gz'))
            path=proof/'domain-portable.log.gz';original=gzip.decompress(path.read_bytes()).decode().splitlines()
            index=next(i for i,s in enumerate(original)if s.startswith('MATCH_SURFACE_DOMAIN '));row=json.loads(original[index].split(' ',1)[1])
            variants=[original[:index]+original[index+1:],original+[original[index]]]
            for key,bad in [('id','unknown'),('archives',True),('parameterization_error',1e-3),('archive_error',float('nan'))]:
                changed=dict(row);changed[key]=bad;lines=original[:];lines[index]='MATCH_SURFACE_DOMAIN '+json.dumps(changed);variants.append(lines)
            for lines in variants:
                path.write_bytes(gzip.compress('\n'.join(lines).encode(),mtime=0))
                with self.assertRaises(ValueError):domains(proof)

if __name__=='__main__':unittest.main()
