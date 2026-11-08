"""Reject incomplete Chamfer comparisons and false history/validity claims."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import chamfer_open_evidence as evidence, report_chamfer_open as report
from .fillet_zero_history import audit
from .check_chamfer_open import ROOT, PROOF


class ChamferOpenTests(unittest.TestCase):
    def test_complete_native_inventory_and_legacy_correspondence(self):
        rows,captures,counts,paths,legacy = evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(69,5))
        self.assertEqual((counts['changed'],counts['geometry_changed'],counts['invalid_outputs']),(61,59,4))
        self.assertEqual(len(legacy),2)
        self.assertTrue(all(r['maximum_scene_difference'] == 0. for r in legacy))
        self.assertFalse(legacy[0]['old_inferred_history']);self.assertTrue(legacy[0]['verified_history'])
        self.assertGreater(len(paths),30)

    def test_equal_replacements_and_noop_have_distinct_history(self):
        rows = evidence.collect(PROOF)[0]
        for key in evidence.EQUAL_RECORDS: self.assertEqual(audit(rows[key],'Chamfer'),dict(recorded=True,geometry_changed=False))
        self.assertEqual(audit(rows['chamfer_open_pilot/open_chamfer_zero_untrimmed'],'Chamfer'),dict(recorded=False,geometry_changed=False))

    def test_public_success_is_insufficient_without_complete_history_pairs(self):
        rows = evidence.collect(PROOF)[0];value = copy.deepcopy(rows['chamfer_open_pilot/open_chamfer_zero_parts'])
        value['undo_observation']['undo_signals'] = [s for s in value['undo_observation']['undo_signals'] if not s['end_undo']]
        with self.assertRaisesRegex(ValueError,'incomplete UndoRedo'): audit(value,'Chamfer')
        value = copy.deepcopy(rows['chamfer_open_pilot/open_chamfer_zero_untrimmed']);value['undo_observation']['undo_succeeded'] = True
        with self.assertRaisesRegex(ValueError,'Undo classification'): audit(value,'Chamfer')

    def test_exact_typed_input_survives_rounded_native_display(self):
        rows = evidence.collect(PROOF)[0];value = rows['chamfer_open_holdouts/open_chamfer_oversized_near']
        typed = value['prescribed_case']['actions'][1]
        self.assertEqual(typed,3.9999999)
        self.assertIn('Distances=4,1 ',value['input_trace'][0]['prompts'][-1])
        line = next(o for o in value['after'] if o['attributes']['name'] == 'guide0')
        self.assertAlmostEqual(line['curve']['domain'][1],4.-typed,places=14)
        self.assertGreater(line['curve']['domain'][1],0.)

    def test_failed_capture_receipt_cannot_publish_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';rows=json.loads(path.read_bytes());rows[0]['exit_status']=1;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'headless capture receipt'): evidence.collect(proof)

    def test_missing_or_nonfinite_geometry_comparison_fails(self):
        if not (PROOF/'open-ui-native.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'open-command-portable.log.gz'
            lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines) if line.startswith('CHAMFER_OPEN_COMMAND '));row=json.loads(lines[index].split(' ',1)[1])
            for error in [float('nan'),1.1e-6]:
                changed=lines.copy();changed[index]='CHAMFER_OPEN_COMMAND '+json.dumps(dict(row,maximum_geometry_error=error))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'): report.comparisons(proof)
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'): report.comparisons(proof)

    def test_native_invalid_joins_require_original_geometry_archive_guards(self):
        if not (PROOF/'open-ui-native.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'open-ui-portable.log.gz'
            lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines) if line.startswith('CHAMFER_OPEN_INVALID_UI '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete invalid UI'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        path=ROOT/'docs/chamfer-open-validation.json'
        if not path.exists(): self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());self.assertEqual(value,report.build(PROOF,value['implementation_revision']))


if __name__ == '__main__': unittest.main()
