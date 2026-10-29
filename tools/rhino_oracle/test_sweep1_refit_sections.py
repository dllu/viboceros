"""Direct section fitting retains the canonical public MakeCompatible reference."""
import copy
import gzip
import hashlib
import json
import unittest

from .generate_sweep1_refit_sections import FAMILY, HERE, request
from .sweep_refit_sections_probe import run


class SectionRefitTests(unittest.TestCase):
    def test_native_cases_and_producers_are_frozen(self):
        fixture = (HERE / ('fixtures/' + FAMILY + '.json')).read_bytes()
        observed = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_bytes())
        self.assertEqual(json.loads(fixture), request())
        self.assertEqual(observed['engine'], 'rhino')
        self.assertEqual(len(observed['results']), 19)
        self.assertEqual([row['id'] for row in observed['results']],
                         [operation['id'] for operation in request()['operations']])
        self.assertEqual(hashlib.sha256(fixture).hexdigest(), observed['provenance']['request_sha256'])
        for digest in observed['provenance']['sources'].values():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)

    def test_direct_native_curves_reproduce_the_existing_compatibility_reference(self):
        prior = json.loads((HERE / 'observations/sweep1_refit_compatibility_stages.json').read_text())
        expected = {}
        for row in prior['results'][1::2]:
            for context in row['value']['refit_sources']:
                if context.get('simplified') is False and 'curves' in context:
                    name = row['id'].split('-angle-')[0] + '-count-' + str(context['source_count'])
                    expected[name] = context['curves']
        actual = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        for row in actual['results'][:16]:
            self.assertEqual(row['value']['curves'], expected[row['id']], row['id'])
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_invalid_inputs_fail_before_accessing_the_host(self):
        valid = request()['operations'][0]
        for update in [dict(sections=[]), dict(parameters=[]), dict(parameters=[True]),
                       dict(parameters=[float('inf')]), dict(refit_tolerance=-1.),
                       dict(refit_tolerance=float('nan')), dict(unexpected=True)]:
            with self.subTest(update=update):
                operation = copy.deepcopy(valid)
                operation.update(update)
                with self.assertRaises(ValueError):
                    run(operation, {}, 1, {})
