"""Owned native rational-offset capture integrity and varied authored inputs."""
import json
import gzip
from pathlib import Path
import shutil
import tempfile
import unittest
from . import offset_srf_rational_evidence as evidence

class RationalOffsetTests(unittest.TestCase):
    def test_every_capture_and_owned_history_is_retained(self):
        rows,captures,counts,_=evidence.collect()
        self.assertEqual((len(rows),len(captures)),(9,3))
        self.assertEqual(counts,dict(observations=9,Success=8,Cancel=1))
        self.assertFalse(rows['offset_rh_cancel']['history_changed'])
        self.assertTrue(rows['offset_rh_cancel']['pending_redo'])

    def test_native_topology_covers_both_skins_and_both_break_axes(self):
        rows=evidence.collect()[0]
        outputs=lambda key:[r for r in rows[key]['after']if not r['source']and not r['guide']]
        self.assertEqual([r['face_count']for r in outputs('offset_rp_both')],[2,2])
        self.assertEqual([r['face_count']for r in outputs('offset_rh_two_axes')],[4])
        self.assertEqual([r['face_count']for r in outputs('offset_rf_two_axes_both')],[4,3])
        guide=next(r for r in rows['offset_rh_reversed_chart']['before']if r['guide'])
        self.assertTrue(guide['brep']['topology']['faces'][0]['reversed'])
        self.assertEqual(guide['surfaces'][0]['domain_u'],[-8.,-6.5])

    def test_desktop_or_failed_capture_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(evidence.PROOF,proof)
            p=proof/'captures.json';rows=json.loads(p.read_bytes())
            for key,value in [('exit_status',1),('command',['desktop-launcher'])]:
                changed=json.loads(json.dumps(rows));changed[0][key]=value;p.write_text(json.dumps(changed))
                with self.assertRaisesRegex(ValueError,'headless capture receipt'):evidence.collect(proof)


    def replay_copy(self,directory):
        proof=Path(directory)/'proof';shutil.copytree(evidence.PROOF,proof);return proof

    def test_nonfinite_geometry_fit_and_changed_partition_witness_are_rejected(self):
        from . import report_offset_srf_rational as report
        if not(evidence.PROOF/'command-native.log.gz').exists():self.skipTest('replays follow capture')
        with tempfile.TemporaryDirectory()as directory:
            proof=self.replay_copy(directory);p=proof/'command-native.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines)if line.startswith('RATIONAL_FINAL_OFFSET_COMMAND ')and report.PARTITION_CASE in line)
            row=json.loads(lines[index].split(' ',1)[1])
            for key,value,reason in [('maximum_geometry_error',float('nan'),'geometry or fit gate'),
                ('maximum_field_error',1.,'geometry or fit gate'),('native_partition_difference',False,'partition witness'),
                ('rational_topology',dict(product=[],native=[]),'partition counts')]:
                changed=lines.copy();changed[index]='RATIONAL_FINAL_OFFSET_COMMAND '+json.dumps(dict(row,**{key:value}))+'\n'
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,reason):report.comparisons(proof)

    def test_missing_and_duplicate_replays_are_rejected(self):
        from . import report_offset_srf_rational as report
        if not(evidence.PROOF/'command-native.log.gz').exists():self.skipTest('replays follow capture')
        with tempfile.TemporaryDirectory()as directory:
            proof=self.replay_copy(directory);p=proof/'command-native.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines)if line.startswith('RATIONAL_FINAL_OFFSET_COMMAND '))
            for changed in [lines[:index]+lines[index+1:],lines+[lines[index]]]:
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_record_reproduces(self):
        from . import report_offset_srf_rational as report
        path=evidence.PROOF.parent/'offset-srf-rational-validation.json'
        if not path.exists():self.skipTest('record follows implementation')
        value=json.loads(path.read_bytes());self.assertEqual(value,report.build(evidence.PROOF,value['implementation_revision']))

if __name__=='__main__':unittest.main()
