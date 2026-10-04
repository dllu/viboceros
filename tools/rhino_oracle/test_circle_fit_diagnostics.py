"""Retained circle/plane fitting diagnostics and private launch constraints."""
import copy
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .circle_fit_diagnostics import request, validate
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


class CircleFitDiagnosticsTests(TestCase):
    def test_closed_inputs_and_private_launch_guards(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/circle_fit_diagnostics.json').read_text()))
        for changes in (dict(op='circle_fit_points'), dict(id='x\n_Delete'), dict(extra=1), dict(points=[[False,0,0]]*3)):
            with self.assertRaises(ValueError):
                validate(dict(q['operations'][0], **changes))
        for scheme, count, headless in ((None,1,True),('VibocerosOracleFit',True,True),('VibocerosOracleFit',2,True),('VibocerosOracleFit',1,False)):
            op = copy.deepcopy(q);op['iterations'] = count
            env = {'DISPLAY':':301'}
            if headless: env['VIBOCEROS_ORACLE_HEADLESS'] = ':301'
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(op,1)
                launch.assert_not_called()

    def test_every_native_circle_uses_the_public_fitted_plane_basis(self):
        for name, count in (('circle_fit_diagnostics',38), ('circle_fit_distant_arcs',3), ('circle_fit_distant_noisy',3)):
            q = json.loads((ROOT/('tools/rhino_oracle/fixtures/'+name+'.json')).read_text())
            r = json.loads((ROOT/('tools/rhino_oracle/observations/'+name+'.json')).read_text())
            self.assertEqual(r['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(r['results']),count)
            self.assertEqual([op['id'] for op in q['operations']],[row['id'] for row in r['results']])
            for op in q['operations']: validate(op)
            for row in r['results']:
                v=row['value'];self.assertTrue(v['circle_success']);self.assertEqual(v['plane_status'],'Success')
                for key in ('x','y','normal'):
                    self.assertEqual(v['circle'][key],v['plane'][key],row['id'])

    def test_provenance_hashes(self):
        record=json.loads((ROOT/'docs/circle-fit-diagnostics-provenance.json').read_text())
        self.assertTrue(record['private_xvfb']);self.assertFalse(record['full_native_parity'])
        for path,expected in record['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected,path)
