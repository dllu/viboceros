"""Retained public SDK evidence for native offset qualification."""
import json
from pathlib import Path
from unittest import TestCase

ROOT=Path(__file__).resolve().parents[2]

class NativeOffsetTests(TestCase):
    def test_warped_sdk_offsets_and_wall_outcomes_are_retained(self):
        fixture=json.loads((ROOT/'tools/rhino_oracle/fixtures/offset_warped_face_geometry.json').read_text())
        capture=json.loads((ROOT/'tools/rhino_oracle/observations/offset_warped_face_geometry.json').read_text())
        self.assertEqual(capture['engine_version'],'8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']],[o['id'] for o in fixture['operations']])
        for op,row in zip(fixture['operations'],capture['results']):
            value=row['value']
            self.assertTrue(value['created'] and value['valid'])
            self.assertEqual(value['solid'],op['solid'])
            self.assertEqual(value['faces'],6 if op['solid'] else 2 if op['both_sides'] else 1)
            if op['solid']:self.assertGreater(value['admitted_volume'],0.)

    def test_box_sdk_empty_wall_results_remain_separate_from_offsets(self):
        fixture=json.loads((ROOT/'tools/rhino_oracle/fixtures/offset_box_geometry.json').read_text())
        capture=json.loads((ROOT/'tools/rhino_oracle/observations/offset_box_geometry.json').read_text())
        self.assertEqual([r['id'] for r in capture['results']],[o['id'] for o in fixture['operations']])
        for op,row in zip(fixture['operations'],capture['results']):
            value=row['value']
            if op['solid']:
                self.assertFalse(value['created'])
                self.assertEqual(value['bodies'],[])
            else:
                self.assertTrue(value['created'])
                self.assertEqual(len(value['bodies']),1)
                body=value['bodies'][0]
                self.assertTrue(body['valid'] and body['solid'])
                self.assertAlmostEqual(body['admitted_volume'],(2.+2.*op['distance'])**3)
