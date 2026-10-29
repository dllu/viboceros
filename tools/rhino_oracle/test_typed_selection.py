"""Frozen command selection outcomes, source invariants and bounded recipes."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from .client import OracleClient, OracleProtocolError
from .typed_selection_probe import request, validate
from .generate_typed_selection_empty import request as empty_request
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


class TypedSelectionTests(unittest.TestCase):
    def test_live_commands_require_private_dedicated_single_iteration_batches(self):
        ordinary=request()
        repeated=copy.deepcopy(ordinary);repeated['iterations']=2
        mixed=copy.deepcopy(ordinary)
        tangent=json.loads((HERE/'fixtures/curve_tangents.json').read_text())['operations'][0]
        mixed['operations'].append(tangent)
        for query,scheme in [(ordinary,None),(repeated,'VibocerosOracleTypeTest'),(mixed,'VibocerosOracleTypeTest')]:
            with (self.subTest(scheme=scheme,iterations=query['iterations']),
                  mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                  mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                  mock.patch('tools.rhino_oracle.client._run_logged') as launch,
                  self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(query)
            launch.assert_not_called()

    def test_fixed_recipe_validation_and_real_job_staging(self):
        query=request()
        validate_operations(query)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {'typed_selection_probe.py','join_probe.py','merge_edges_probe.py'})
        for field,value in [('command','SelAll'),('scenario','unknown'),('id','mismatched')]:
            op=copy.deepcopy(query['operations'][0]);op[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(query['operations'][0]);op['extra']=True
        with self.assertRaises(ValueError):validate(op)

    def test_native_requests_and_producers_are_immutable(self):
        for family,build,count in [('typed_selection',request,21),('typed_selection_empty',empty_request,6)]:
            fixture=(HERE/('fixtures/'+family+'.json')).read_bytes()
            native=json.loads((HERE/('observations/'+family+'.json')).read_text())
            self.assertEqual(json.loads(fixture),build())
            self.assertEqual(native['engine'],'rhino')
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(fixture).hexdigest())
            self.assertEqual(len(native['results']),count)
            self.assertEqual([r['id']for r in native['results']],[op['id']for op in build()['operations']])
            for digest in native['provenance']['sources'].values():
                content=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(),digest)
            for row in native['results']:
                self.assertTrue(row['value']['unchanged'])
                self.assertTrue(row['value']['redo_preserved'])
                self.assertEqual(len(row['value']['types']),13)
