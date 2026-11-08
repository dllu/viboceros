"""Reject incomplete native captures, comparison gaps and unretained observer failures."""
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import fillet_continuation_evidence as evidence, report_fillet_continuation as report
from .fillet_continuation_sdk_evidence import collect as sdk
from .check_fillet_continuation import ROOT, PROOF


class FilletContinuationTests(unittest.TestCase):
    def test_all_outcomes_and_frozen_recipe_imports_are_retained(self):
        rows, captures, counts, paths = evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(44,4))
        self.assertEqual(counts,evidence.EXPECTED_COUNTS)
        self.assertGreater(len(paths),25)
        for family in evidence.FAMILIES:
            v=json.loads(gzip.decompress((evidence.HERE/('observations/'+family+'.json.gz')).read_bytes()))
            self.assertTrue({'fillet_continuation_cases.py','fillet_command.py','fillet_command_probe.py'}.issubset(v['provenance']['sources']))

    def test_failed_capture_cannot_contribute_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'captures.json';v=json.loads(p.read_bytes());v[0]['exit_status']=1;p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'headless capture'):evidence.collect(proof)

    def test_sdk_controls_and_corrected_failed_attempt_remain_separate(self):
        controls,paths=sdk(PROOF,evidence.collect(PROOF)[0])
        self.assertEqual((controls['observations'],controls['extended_contacts'],controls['unchanged_inputs'],controls['opposed_contacts']),(8,8,8,5))
        self.assertLessEqual(controls['maximum_analytic_difference'],1e-6)
        self.assertGreater(len(paths),5)
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'failed-sdk.json';v=json.loads(p.read_bytes());v['native_observations']=1;p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'failed SDK receipt'):sdk(proof,evidence.collect(PROOF)[0])

    def test_missing_and_nonfinite_comparisons_fail_the_gate(self):
        if not (PROOF/'continuation-command-portable.log.gz').exists():self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'continuation-command-portable.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines) if line.startswith('FILLET_CONTINUATION_COMMAND '));row=json.loads(lines[i].split(' ',1)[1])
            for value in [float('nan'),1.1e-6]:
                changed=lines.copy();changed[i]='FILLET_CONTINUATION_COMMAND '+json.dumps(dict(row,maximum_geometry_error=value))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'):report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_missing_rejection_receipt_fails_the_gate(self):
        if any(not (PROOF/('continuation-'+stage+'-'+mode+'.log.gz')).exists() for stage in ['command','ui'] for mode in ['portable','native']):self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'continuation-command-portable.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines) if line.startswith('FILLET_CONTINUATION_REJECTION '))
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete rejection coverage'):report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        p=ROOT/'docs/fillet-continuation-validation.json'
        if not p.exists():self.skipTest('record follows implementation commit')
        value=json.loads(p.read_bytes());self.assertEqual(value,report.build(PROOF,value['implementation_revision']))


if __name__=='__main__':unittest.main()
