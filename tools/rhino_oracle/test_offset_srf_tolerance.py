"""Capture inventory, numeric getter evidence and reproducible finite checks."""
import gzip
import json
import shutil
import tempfile
import unittest
from . import offset_srf_tolerance_evidence as evidence,report_offset_srf_tolerance as report
from .check_offset_srf_tolerance import ROOT,PROOF

class ToleranceOffsetTests(unittest.TestCase):
    def test_all_captures_and_diagnostics_are_retained(self):
        rows,captures,counts,paths=evidence.collect()
        self.assertEqual((len(rows),len(captures),len(paths)),(30,3,41))
        self.assertEqual(counts,dict(observations=30,Success=24,Cancel=6,qualified=28,diagnostics=2))

    def test_defaults_custom_memory_units_and_single_escape(self):
        r=evidence.collect()[0]
        for key,value in [('offset_tp_document',.003),('offset_th_explicit_new_document',.003),
            ('offset_th_units',.001),('offset_tf_remember_document',.0002),('offset_tf_default_follow',.004)]:
            self.assertEqual(report.requested(r[key]),value)
        for suffix in ['offset_th_getter_escape','offset_tf_postselect_cancel']:
            v=r[suffix];self.assertFalse(v['success']);self.assertFalse(v['history_changed'])
            self.assertEqual(len(v['external_keys']),1)
        self.assertEqual(r['offset_th_after_escape']['input_trace'][0]['errors'],[])

    def test_failed_headless_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=ROOT/directory/'proof';shutil.copytree(PROOF,proof)
            p=proof/'captures.json';v=json.loads(p.read_bytes());v[0]['exit_status']=1;p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'headless capture receipt'):evidence.collect(proof)

    def test_nonfinite_fit_or_missing_duplicate_replays_are_rejected(self):
        if not(PROOF/'command-native.log.gz').exists():self.skipTest('replays follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=ROOT/directory/'proof';shutil.copytree(PROOF,proof);p=proof/'command-native.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,l in enumerate(lines)if l.startswith('TOLERANCE_OFFSET_COMMAND '));row=json.loads(lines[i].split(' ',1)[1])
            for key,value in [('maximum_geometry_error',float('nan')),('maximum_geometry_error',-1.),('maximum_field_error',1.)]:
                changed=lines.copy();changed[i]='TOLERANCE_OFFSET_COMMAND '+json.dumps(dict(row,**{key:value}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry or fit gate'):report.comparisons(proof)
            for changed in [lines[:i]+lines[i+1:],lines+[lines[i]]]:
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_record_reproduces(self):
        path=ROOT/'docs/offset-srf-tolerance-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        v=json.loads(path.read_bytes());self.assertEqual(v,report.build(PROOF,v['implementation_revision']))

if __name__=='__main__':unittest.main()
