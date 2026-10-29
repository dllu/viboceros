"""Integrity and scope of the retained closed-ring Loft command evidence."""
from tools.verify_provenance import verify_measurement
import gzip
import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'
FAMILY = 'loft_splitting_closed_orientation_regular'


class LoftCommandOptionsTests(unittest.TestCase):
    def test_native_closed_ring_capture_and_archived_producers(self):
        fixture = HERE / ('fixtures/' + FAMILY + '.json')
        query = json.loads(fixture.read_text())
        data = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),
                         data['provenance']['request_sha256'])
        for name, digest in data['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(),
                             digest, name)
        expected_ids = {'loft-closed-regular-{}-{}-{}'.format(shape, style, reverse)
                        for shape in ['circle-peak', 'square-centered']
                        for style in ['normal', 'straight'] for reverse in [0, 1]}
        self.assertEqual({op['id'] for op in query['operations']}, expected_ids)
        self.assertEqual(len(query['operations']), 8)
        self.assertEqual([row['id'] for row in data['results']],
                         [op['id'] for op in query['operations']])
        for op, row in zip(query['operations'], data['results']):
            self.assertIs(op['closed'], True)
            self.assertIs(op['crease_splitting'], False)
            self.assertIs(op['split_at_tangents'], False)
            self.assertEqual(len(op['curves']), 3)
            value = row['value']
            self.assertEqual(len(value['after']), 4)
            self.assertEqual(len(value['undo']), 3)
            self.assertEqual(value['after'][-1], value['redo'][-1])
            for phase in ['after', 'undo', 'redo']:
                self.assertTrue(all(source['unchanged'] for source in value[phase][:3]))
            brep = value['after'][-1]
            self.assertIs(brep['is_solid'], True)
            self.assertEqual(brep['solid_orientation'], 1)
            self.assertEqual(brep['face_count'], 1)
            self.assertIs(brep['faces'][0]['reversed'], True)
            self.assertEqual(len(brep['faces'][0]['samples']), 25)
            self.assertTrue(any(event['name'] == 'Loft' and event['result'] == 'Success'
                                for event in value['events']))

    def test_qualification_source_bindings(self):
        record = json.loads((ROOT / 'docs/loft-command-options.json').read_text())
        self.assertEqual(record['native_closed_ring_cases'], 8)
        self.assertIs(record['full_rhino_parity'], False)
        self.assertIs(record['continuous_error_certificate'], False)
        self.assertIs(record['performance_qualified'], False)
        self.assertGreater(verify_measurement(record), 0)


if __name__ == '__main__':
    unittest.main()
