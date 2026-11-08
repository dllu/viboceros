"""Owned native rational-offset capture integrity and varied authored inputs."""
import json
import shutil
import tempfile
import unittest
from . import offset_srf_rational_evidence as evidence

class RationalOffsetTests(unittest.TestCase):
    def test_every_capture_and_owned_history_is_retained(self):
        rows,captures,counts,_=evidence.collect()
        self.assertEqual((len(rows),len(captures)),(7,2))
        self.assertEqual(counts,dict(observations=7,Success=6,Cancel=1))
        self.assertFalse(rows['offset_rh_cancel']['history_changed'])
        self.assertTrue(rows['offset_rh_cancel']['pending_redo'])

    def test_native_topology_covers_both_skins_and_both_break_axes(self):
        rows=evidence.collect()[0]
        outputs=lambda key:[r for r in rows[key]['after']if not r['source']and not r['guide']]
        self.assertEqual([r['face_count']for r in outputs('offset_rp_both')],[2,2])
        self.assertEqual([r['face_count']for r in outputs('offset_rh_two_axes')],[4])
        guide=next(r for r in rows['offset_rh_reversed_chart']['before']if r['guide'])
        self.assertTrue(guide['brep']['topology']['faces'][0]['reversed'])
        self.assertEqual(guide['surfaces'][0]['domain_u'],[-8.,-6.5])

    def test_desktop_or_failed_capture_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=evidence.PROOF.parent/directory/'proof';shutil.copytree(evidence.PROOF,proof)
            p=proof/'captures.json';rows=json.loads(p.read_bytes())
            for key,value in [('exit_status',1),('command',['desktop-launcher'])]:
                changed=json.loads(json.dumps(rows));changed[0][key]=value;p.write_text(json.dumps(changed))
                with self.assertRaisesRegex(ValueError,'headless capture receipt'):evidence.collect(proof)

if __name__=='__main__':unittest.main()
