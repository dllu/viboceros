"""Immutable Auto Range evidence, validators and owned panel completion traces."""
import copy
import gzip
import hashlib
import json
import unittest
from .curvature_analysis_comparison import canonical
from .curvature_auto_sdk_probe import validate, STYLES
from .generate_curvature_auto import HERE, request, panel_request, SAMPLES


class CurvatureAutoTests(unittest.TestCase):
    def test_native_capture_provenance_and_settings_preservation(self):
        samples = json.loads(SAMPLES.read_text())
        for family, recipe in (('curvature_auto_sdk', request), ('curvature_auto_panel', panel_request)):
            path = HERE/'fixtures'/(family+'.json')
            q = json.loads(path.read_text())
            n = json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(q, recipe())
            self.assertEqual(n['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, sha in n['provenance']['sources'].items():
                raw = gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), sha, name)
                if name.endswith(('.3dm', 'samples.json')):
                    self.assertEqual(raw, (HERE/name).read_bytes(), name)
            self.assertEqual(len(q['operations']), len(n['results']))
            for op, row in zip(q['operations'], n['results']):
                self.assertEqual(row['id'], op['id'])
                self.assertNotIn('error', row)
                v = row['value']
                if family.endswith('panel'):
                    canonical(op, v, True)
                    if 'Hide' in op['steps']:
                        self.assertEqual(v['traces'][2]['settings'], v['traces'][1]['settings'])
                    continue
                i = STYLES.index(op['style'])
                initial = op.get('initial', [[-.1,.1],[-.1,.1],[0,1],[0,1]])
                assets = ([op['asset']] if op['asset'] is not None else []) + op.get('companions', [])
                self.assertEqual(v['success'], bool(assets))
                self.assertEqual(v['count'], sum(len(samples[a]) for a in assets))
                self.assertEqual(v['style'], op['style'])
                for j in range(4):
                    if j != i or not assets:
                        self.assertEqual(v['ranges'][j], initial[j])

    def test_invalid_assets_ranges_and_identifiers_are_rejected(self):
        op = request()['operations'][0]
        for changes in ({'asset':'../file'}, {'asset':False}, {'companions':['../file']},
                        {'id':'bad/path'}, {'style':'Bad'}, {'initial':[[0,0]]*4},
                        {'initial':[[float('nan'),1]]*4}, {'initial':[[False,1]]*4}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(op), **changes))


if __name__ == '__main__':
    unittest.main()
