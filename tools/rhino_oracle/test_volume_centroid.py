"""Volume command replay must not replace raw API errors with command targets."""
import copy
import hashlib
import json
import unittest
from pathlib import Path
from .area_centroid_probe import validate
from .area_centroid_replay import prepare
from .client import OracleProtocolError
from .references.volume_centroid import request

ROOT = Path(__file__).resolve().parents[2]


def inputs():
    return [json.loads((ROOT/('tools/rhino_oracle/'+directory+'/volume_centroid.json')).read_text())
            for directory in ('fixtures','observations')]


class VolumeCentroidTests(unittest.TestCase):
    def test_sources_regenerate_without_observations(self):
        data, observed = inputs()
        self.assertEqual(data, request())
        self.assertEqual(len(data['operations']), 26)
        for source_api in (False, True):
            native, _ = prepare(data, observed, measure='volume', source_api=source_api)
            self.assertEqual(native, data)
            changed = copy.deepcopy(observed)
            for row in changed['results']:
                for point in row['value']['points']: point['point'] = [123.,456.,789.]
                for mass in row['value']['source_properties']:
                    if mass is not None: mass['centroid'] = [123.,456.,789.]
            self.assertEqual(prepare(data, changed, measure='volume', source_api=source_api)[0], native)

    def test_command_and_source_api_scopes_are_explicitly_separate(self):
        data, observed = inputs()
        _, command = prepare(data, observed, measure='volume')
        _, api = prepare(data, observed, measure='volume', source_api=True)
        for raw, a, b in zip(observed['results'], command['results'], api['results']):
            self.assertEqual(set(a['value']), {'points','selected','succeeded'})
            self.assertEqual(b['value'], dict(properties=raw['value']['source_properties']))
        rows = {r['id']:r['value'] for r in observed['results']}
        self.assertTrue(rows['equal-opposite-volumes']['succeeded'])
        self.assertEqual(rows['equal-opposite-volumes']['points'], [])
        self.assertEqual(rows['equal-opposite-volumes']['selected'], [0,1])
        self.assertEqual(rows['post-equal-opposite-volumes']['selected'], [])
        self.assertFalse(rows['line-only']['succeeded'])

    def test_negative_mesh_centroid_getter_does_not_replace_signed_first_moments(self):
        _, observed = inputs()
        rows = {r['id']:r['value'] for r in observed['results']}
        row = rows['reversed-tetrahedron']
        self.assertEqual(row['source_properties'], [dict(volume=-10.,centroid=[0.,0.,0.])])
        self.assertEqual(row['source_first_moments'], [[-7.5,-10.,-12.5]])
        self.assertEqual(row['points'][0]['point'], [.75,1.,1.25])
        row = rows['brep-paraboloid-capped-inward']
        self.assertLess(row['source_properties'][0]['volume'],0.)
        self.assertGreater(row['properties'][0]['volume'],0.)
        self.assertEqual(row['insertions'], ['reversed'])

    def test_invalid_scope_source_and_diagnostic_fields_fail(self):
        data, observed = inputs()
        for operation in [dict(data['operations'][0], selected=[True]),dict(data['operations'][0], extra=True)]:
            with self.assertRaises(ValueError): validate(operation, 'volume')
        with self.assertRaises(OracleProtocolError): prepare(data, observed, measure='volume', tight_api=True)
        for change in [dict(source_first_moments=[]),dict(source_first_moments=[[0.,0.,float('inf')]])]:
            wrong = copy.deepcopy(observed); wrong['results'][0]['value'].update(change)
            with self.assertRaises(OracleProtocolError): prepare(data, wrong, measure='volume')
        wrong = copy.deepcopy(observed)
        wrong['results'][0]['value']['inputs'][0]['vertices'][0][0] = 2.
        with self.assertRaises(OracleProtocolError): prepare(data, wrong, measure='volume')

    def test_open_probe_timeout_is_not_a_rejection_or_success_observation(self):
        failed = ROOT/'tools/rhino_oracle/observations/volume_centroid_open_timeout.txt'
        self.assertIn('operation open-tetra: start', failed.read_text())
        self.assertIn('did not respond within 180 seconds', failed.read_text())
        with self.assertRaises(ValueError): json.loads(failed.read_text())

    def test_provenance_hashes(self):
        provenance = json.loads((ROOT/'docs/volume-centroid-provenance.json').read_text())
        for path, digest in provenance['retained_file_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest)


if __name__ == '__main__': unittest.main()
