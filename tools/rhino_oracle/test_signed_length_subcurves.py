"""Raw public SDK geometry and retained inline-getter diagnostics."""
import hashlib
import json
import math
from pathlib import Path
from unittest import TestCase

ROOT=Path(__file__).resolve().parents[2]
def read(path): return json.loads((ROOT/path).read_text())

class SignedLengthSubcurveTests(TestCase):
    def test_original_sources_and_every_paired_native_station(self):
        fixture=read('tools/rhino_oracle/fixtures/signed_length_subcurves.json')
        native=read('tools/rhino_oracle/observations/signed_length_subcurves.json')
        local=read('docs/signed-length-subcurves-local.json')
        self.assertEqual((len(fixture['operations']),len(native['results']),len(local['results'])),(20,20,20))
        available=stations=0
        for f,n,l in zip(fixture['operations'],native['results'],local['results']):
            self.assertEqual((n['id'],l['id']),(f['id'],f['id']))
            a,b=n['value'],l['value']
            self.assertTrue(a['source_unchanged']);self.assertTrue(b['source_unchanged'])
            self.assertEqual(a['available'],b['available'])
            if not a['available']:
                self.assertIsNone(a['curve']);self.assertIsNone(b['curve']);continue
            available+=1
            self.assertEqual((len(a['curve']['samples']),len(b['curve']['samples'])),(33,33))
            for p,q in zip(a['curve']['samples'],b['curve']['samples']):
                stations+=1
                self.assertLess(math.dist(p,q),1e-6,n['id'])
        self.assertEqual((available,stations),(16,528))
        self.assertEqual(native['engine_version'],'8.32.26160.13001')

    def test_getter_diagnostics_keep_failures_and_missing_temporary_outputs(self):
        scripted=read('tools/rhino_oracle/observations/subcurve_length_scripted_diagnostic.json')
        cursor=read('tools/rhino_oracle/observations/subcurve_length_cursor_diagnostic.json')
        self.assertEqual((len(scripted['results']),len(cursor['results'])),(16,3))
        self.assertTrue(any(not r['value']['success'] for r in scripted['results']))
        for record in scripted['results']+cursor['results']:
            v=record['value']
            self.assertTrue(all(o['name'] is not None for o in v['after'] if o['source'] is None))
            self.assertFalse(v['command_active'])
            originals={o['source']:o for o in v['before']}
            for o in v['after']:
                if o['source'] is not None:self.assertEqual(o,dict(originals[o['source']],selected=o['selected']))

    def test_provenance_preserves_canonical_sdk_and_historical_scope(self):
        p=read('docs/signed-length-subcurves-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['sdk_recipes'],p['returned_curves'],p['unavailable_cases']),(20,16,4))
        self.assertEqual(p['settings_scheme'],'VibocerosOracleSignedLengthFinal20261007')
        self.assertFalse(p['inline_length_parity'])
        self.assertFalse(p['historical_getter_producers_retained'])
        for path,digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
