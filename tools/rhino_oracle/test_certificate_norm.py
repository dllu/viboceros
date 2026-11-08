"""Benchmark integrity and reproducible coefficient norm qualification."""
import gzip
import json
import shutil
import tempfile
import unittest
from . import report_certificate_norm as report
from .check_certificate_norm import ROOT,PROOF

class CertificateNormTests(unittest.TestCase):
    def copy_proof(self,directory):
        proof=ROOT/directory/'proof';shutil.copytree(PROOF,proof);return proof

    def test_complete_brep_outputs_match_for_both_signed_edits(self):
        data=report.benchmark()
        self.assertEqual([r['distance']for r in data['cases']],[-.3,.5])
        self.assertEqual(len(data['current_source_sha256']),2)
        self.assertEqual(len(data['producer_sha256']),2)

    def test_changed_geometry_nonfinite_timing_and_missing_case_are_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=self.copy_proof(directory);path=proof/'benchmark.log.gz';data=report.benchmark()
            for key,value,reason in [('candidate_geometry_sha256','0'*64,'changed benchmark geometry'),
                ('candidate_seconds',float('nan'),'benchmark timing'),('candidate_seconds',-1.,'benchmark timing')]:
                changed=json.loads(json.dumps(data));changed['cases'][0][key]=value
                path.write_bytes(gzip.compress(('CERTIFICATE_NORM_BENCHMARK '+json.dumps(changed)+'\n').encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,reason):report.benchmark(proof)
            data['cases'].pop()
            path.write_bytes(gzip.compress(('CERTIFICATE_NORM_BENCHMARK '+json.dumps(data)+'\n').encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'benchmark coverage'):report.benchmark(proof)

    def test_duplicate_benchmark_is_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=self.copy_proof(directory);path=proof/'benchmark.log.gz'
            text=gzip.decompress(path.read_bytes());path.write_bytes(gzip.compress(text+text,mtime=0))
            with self.assertRaisesRegex(ValueError,'benchmark coverage'):report.benchmark(proof)

    def test_record_reproduces(self):
        path=ROOT/'docs/certificate-norm-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());self.assertEqual(value,report.build(PROOF,value['implementation_revision']))

if __name__=='__main__':unittest.main()
