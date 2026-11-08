"""Reject incomplete comparisons and inferred history in open zero Fillet proof."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import fillet_open_zero_evidence as evidence, report_fillet_open_zero as report
from .fillet_zero_history import audit
from .check_fillet_open_zero import ROOT, PROOF


class FilletOpenZeroTests(unittest.TestCase):
    def test_complete_native_inventory_and_legacy_corrections(self):
        rows, captures, counts, paths, corrections = evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(49,5))
        self.assertEqual((counts['changed'],counts['geometry_changed'],counts['invalid_outputs']),(42,39,0))
        self.assertEqual(len(corrections),2)
        self.assertTrue(all(c['verified_record'] and c['maximum_scene_difference'] == 0. for c in corrections))
        self.assertGreater(len(paths),40)

    def test_equal_geometry_has_real_history_pairs(self):
        rows = evidence.collect(PROOF)[0]
        for key in evidence.EQUAL_RECORDS:
            self.assertEqual(audit(rows[key]),dict(recorded=True,geometry_changed=False))
        self.assertEqual(audit(rows['fillet_open_zero_pilot/open_zero_parallel_untrimmed']),dict(recorded=False,geometry_changed=False))

    def test_command_success_cannot_replace_an_undo_pair(self):
        rows = evidence.collect(PROOF)[0]
        value = copy.deepcopy(rows['fillet_open_zero_pilot/open_zero_meeting_parts'])
        value['undo_observation']['undo_signals'] = [s for s in value['undo_observation']['undo_signals'] if not s['end_undo']]
        with self.assertRaisesRegex(ValueError,'incomplete UndoRedo'): audit(value)
        value = copy.deepcopy(rows['fillet_open_zero_pilot/open_zero_parallel_untrimmed'])
        value['undo_observation']['undo_succeeded'] = True
        with self.assertRaisesRegex(ValueError,'Undo classification'): audit(value)

    def test_failed_capture_and_unusable_pick_cannot_contribute(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof = Path(directory)/'proof'; shutil.copytree(PROOF,proof)
            path = proof/'failed-overlaps.json'; receipt = json.loads(path.read_bytes())
            receipt['native_observations'] = 1; path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError,'failed capture receipt'): evidence.collect(proof)
            shutil.copyfile(PROOF/'failed-overlaps.json',path)
            path = proof/'selection-receipt.json'; receipt = json.loads(path.read_bytes())
            receipt['usable_source_pick'] = True; path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError,'private menu intervention'): evidence.collect(proof)

    def test_missing_nonfinite_and_excessive_comparisons_fail(self):
        if not (PROOF/'open-zero-ui-native.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof = Path(directory)/'proof'; shutil.copytree(PROOF,proof)
            path = proof/'open-zero-command-portable.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i,line in enumerate(lines) if line.startswith('FILLET_OPEN_ZERO_COMMAND '))
            row = json.loads(lines[index].split(' ',1)[1])
            for error in [float('nan'),1.1e-6]:
                changed = lines.copy(); changed[index] = 'FILLET_OPEN_ZERO_COMMAND '+json.dumps(dict(row,maximum_geometry_error=error))+'\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'): report.comparisons(proof)
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'): report.comparisons(proof)

    def test_rejection_guards_and_actual_history_are_required(self):
        if not (PROOF/'open-zero-ui-native.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof = Path(directory)/'proof'; shutil.copytree(PROOF,proof)
            path = proof/'open-zero-command-portable.log.gz'
            original = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i,line in enumerate(original) if line.startswith('FILLET_OPEN_ZERO_REJECTION '))
            path.write_bytes(gzip.compress(''.join(original[:index]+original[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete API rejection'): report.comparisons(proof)
            lines = original.copy(); index = next(i for i,line in enumerate(lines) if line.startswith('FILLET_OPEN_ZERO_COMMAND '))
            row = json.loads(lines[index].split(' ',1)[1]); row['changed'] = not row['changed']
            lines[index] = 'FILLET_OPEN_ZERO_COMMAND '+json.dumps(row)+'\n'
            path.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'history or archive'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        path = ROOT/'docs/fillet-open-zero-validation.json'
        if not path.exists(): self.skipTest('record follows implementation commit')
        value = json.loads(path.read_bytes())
        self.assertEqual(value,report.build(PROOF,value['implementation_revision']))

    def test_native_boundary_producer_regression_is_required(self):
        if not (PROOF/'native-producers.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof = Path(directory)/'proof'; shutil.copytree(PROOF,proof)
            path = proof/'native-producers.log.gz'
            text = gzip.decompress(path.read_bytes()).decode().replace('BOUNDED_LINE_OVERLAPS 64','')
            path.write_bytes(gzip.compress(text.encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'bounded native producer'): report.comparisons(proof)


if __name__ == '__main__': unittest.main()
