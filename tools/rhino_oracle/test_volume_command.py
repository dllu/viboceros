"""Scalar command targets are real history results, never collection API values."""
import copy
import hashlib
import json
import unittest
from pathlib import Path
from .area_centroid_probe import validate
from .client import OracleProtocolError
from .references.volume_command import request
from .volume_command_replay import history_volume, prepare
from .volume_confirmation import VolumeConfirmation

ROOT = Path(__file__).resolve().parents[2]


def inputs():
    return [json.loads((ROOT / ('tools/rhino_oracle/'+d+'/volume_command.json')).read_text())
            for d in ('fixtures', 'observations')]


class VolumeCommandTests(unittest.TestCase):
    def test_retained_capture_provenance(self):
        metadata = json.loads((ROOT/'docs/volume-command-provenance.json').read_text())
        for path, digest in metadata['retained_file_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest)
        data, observed = inputs()
        self.assertEqual(sum(op['op']=='volume_command' for op in data['operations']), metadata['scalar_volume_commands'])
        self.assertEqual(sum(row['value']['succeeded'] for row in observed['results']), metadata['successful_commands'])
        report = json.loads((ROOT/'docs/volume-command-comparison.json').read_text())
        self.assertEqual(len(report['operations']), metadata['native_comparison_cases'])
        self.assertEqual(sum(row['passed'] for row in report['operations']), metadata['native_comparison_matches'])
        self.assertFalse(report['passed'])  # Retained counterexamples are not a passing parity claim.

    def test_source_generator_and_dedicated_owned_input(self):
        data, observed = inputs()
        self.assertEqual(data, request())
        self.assertEqual(len(data['operations']), 80)
        VolumeConfirmation(data)
        native, evidence = prepare(data, observed)
        self.assertEqual(native, data)
        self.assertEqual(len(evidence['results']), 80)
        changed = copy.deepcopy(observed)
        for row in changed['results']:
            mass = row['value']['collection']
            if mass is not None: mass['properties']['volume'] = 123456.
        self.assertEqual(prepare(data, changed), (native,evidence))

    def test_actual_history_not_api_defines_scalar_targets(self):
        data, observed = inputs()
        _, evidence = prepare(data, observed)
        rows = {r['id']:r['value'] for r in evidence['results']}
        self.assertEqual(rows['equal-opposite-volumes']['volume'], 0.)
        self.assertEqual(rows['open-surface-pre-yes']['volume'], -2.)
        self.assertIsNone(rows['open-surface-pre-no']['volume'])
        self.assertEqual(rows['unjoined-box-surfaces-yes']['volume'], 24.)
        raw = next(r['value'] for r in observed['results'] if r['id']=='diagnostic-base-volume_centroid_command')
        self.assertEqual(raw['points'][0]['point'], raw['collection']['properties']['centroid'])
        self.assertGreater(abs(raw['points'][0]['point'][0]-raw['properties'][0]['centroid'][0]), 1e14)
        changed = copy.deepcopy(observed)
        changed['results'][0]['value']['history'] = 'Volume = 12 (+/- 1e-08) cubic millimeters'
        self.assertEqual(prepare(data, changed)[1]['results'][0]['value']['volume'], 12.)
        self.assertEqual(prepare(data, changed)[0], data)

    def test_rejects_ambiguous_invalid_or_unsupported_history(self):
        _, observed = inputs()
        value = observed['results'][0]['value']
        for history in ['', value['history']+'\n'+value['history'],
                        'Volume = nan (+/- 1) cubic millimeters',
                        'Volume = 1e999 (+/- 1) cubic millimeters',
                        'Volume = 10 (+/- -1) cubic millimeters',
                        'Volume = 10 (+/- 1) cubic meters']:
            with self.assertRaises(OracleProtocolError): history_volume(dict(value,history=history))
        with self.assertRaises(OracleProtocolError): history_volume(dict(value,succeeded=False))
        with self.assertRaises(OracleProtocolError): history_volume(dict(value,display=dict(precision=6,units='Millimeters')))

    def test_invalid_diagnostics_and_scalar_geometry_do_not_pass(self):
        data, observed = inputs()
        for change in [dict(collection_api=1),dict(op='area_command'),dict(selected=[True])]:
            with self.assertRaises(ValueError): validate(dict(data['operations'][0],**change), 'volume', False)
        for field, value in [('collection', {}), ('display', {}), ('points', [{}])]:
            changed = copy.deepcopy(observed)
            changed['results'][0]['value'][field] = value
            with self.assertRaises(OracleProtocolError): prepare(data, changed)


if __name__ == '__main__': unittest.main()
