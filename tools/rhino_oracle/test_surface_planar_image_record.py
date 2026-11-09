"""Reject altered image bounds, workflow categories and producer hashes."""
import copy
import json
import unittest
from . import report_surface_planar_image as record


class PlanarImageRecordTests(unittest.TestCase):
    def test_record_reproduces_stored_summary(self):
        stored = json.loads((record.ROOT / 'docs/surface-planar-image-validation.json').read_bytes())
        self.assertEqual(record.report(), stored)

    def test_image_rows_keep_the_strict_gate_and_exact_inventory(self):
        captures = [json.loads(record.read(record.ROOT / ('tools/rhino_oracle/observations/' + f + '.json.gz')))
                    for f in record.FIT_FAMILIES]
        log = record.read(record.PROOF / 'portable.log.gz').decode()
        rows = record.rows(log, 'PLANAR_FIT_IMAGE ')
        def changed_log(changed):
            return '\n'.join('PLANAR_FIT_IMAGE ' + json.dumps(r) for r in changed) + '\n' + '\n'.join(
                line for line in log.splitlines() if line.startswith('PLANAR_FIT_IMAGE_TOTAL '))
        self.assertEqual(record.summarize_images(captures, log)['certified_planar_images'], 30)
        changed = copy.deepcopy(rows)
        next(r for r in changed if r['planar_fixture'])['image_bound'] = 1e-8
        with self.assertRaises(AssertionError):record.summarize_images(captures, changed_log(changed))
        changed = copy.deepcopy(rows);changed[0]['id'] = changed[1]['id']
        with self.assertRaises(AssertionError):record.summarize_images(captures, changed_log(changed))

    def test_active_commands_and_changed_request_are_rejected(self):
        queries = [json.loads((record.ROOT / ('tools/rhino_oracle/fixtures/' + f + '.json')).read_bytes())
                   for f in record.COMMAND_FAMILIES]
        captures = [json.loads(record.read(record.ROOT / ('tools/rhino_oracle/observations/' + f + '.json.gz')))
                    for f in record.COMMAND_FAMILIES]
        log = record.read(record.PROOF / 'portable.log.gz').decode()
        self.assertEqual(record.summarize_commands(queries, captures, log)['accepted'], 9)
        changed = copy.deepcopy(captures);changed[0]['results'][0]['value']['command']['active'] = True
        with self.assertRaises(AssertionError):record.summarize_commands(queries, changed, log)
        with self.assertRaises(AssertionError):record.audit_producer(captures[0], b'{}')


if __name__ == '__main__':unittest.main()
