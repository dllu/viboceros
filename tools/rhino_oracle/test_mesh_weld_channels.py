"""Fixed producer admission and complete, explicit mesh welding comparisons."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from . import mesh_weld_channels_cases as cases
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class MeshWeldChannels(unittest.TestCase):
    def test_native_requests_inputs_and_frozen_producers(self):
        counts=[]
        for suffix,make,scenes in [('',cases.request,cases.CASES),('_followup',cases.followup_request,cases.FOLLOWUP),
                                   ('_prefixes',cases.prefixes_request,cases.PREFIXES),('_pairs',cases.pairs_request,cases.PAIRS)]:
            family='mesh_weld_channels'+suffix
            path=HERE/'fixtures'/(family+'.json')
            query=json.loads(path.read_text());self.assertEqual(query,make());validate_operations(query)
            native=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in native['results']],[r['id'] for r in query['operations']])
            for row in native['results']:
                self.assertNotIn('error',row);self.assertEqual(row['value']['input'],scenes[row['id']])
                self.assertEqual(row['value']['before']['vertices'],scenes[row['id']]['source']['vertices'])
                self.assertEqual(row['value']['before']['faces'],scenes[row['id']]['source']['faces'])
            for name,digest in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            counts.append(len(native['results']))
        self.assertEqual(counts,[18,13,10,15])

    def test_unowned_mutated_mixed_or_unbounded_capture_is_rejected(self):
        query=cases.request()
        for extra in [dict(id='arbitrary'),dict(case=[]),dict(angle=1),dict(script='arbitrary')]:
            bad=copy.deepcopy(query);bad['operations'][0].update(extra)
            with self.assertRaises(ValueError):validate_operations(bad)
        with patch.dict('os.environ',{'DISPLAY':':997','VIBOCEROS_ORACLE_HEADLESS':':997'}),patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
            client=OracleClient(settings_scheme='VibocerosOracleWeldValidation')
            for change in ['boolean','repeat','duplicate','mixed','large']:
                bad=copy.deepcopy(query)
                if change=='boolean':bad['iterations']=True
                elif change=='repeat':bad['iterations']=2
                elif change=='duplicate':bad['operations'].append(bad['operations'][0])
                elif change=='mixed':bad['operations'].append(cases.followup_request()['operations'][0])
                else:bad['operations']*=4
                with self.assertRaises(OracleProtocolError):client.run_rhino(bad)
            launch.assert_not_called()

    def test_report_requires_all_comparisons_and_explicit_limits(self):
        from .report_mesh_weld_channels import ROOT,summarize
        report=json.loads((ROOT/'docs/mesh-weld-channels-qualification.json').read_text())
        lines=['MESH_WELD_CHANNEL_NATIVE '+json.dumps(r) for r in report['cases']]
        self.assertEqual(summarize('\n'.join(lines)),report)
        self.assertEqual((report['compared_cases'],report['qualified_cases'],len(report['diagnostic_cases'])),(47,24,9))
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for bad in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(bad))
        for field,value in [('qualified',1),('vertices',True),('faces',-1),('differences',['unmeasured'])]:
            rows=copy.deepcopy(report['cases']);rows[0][field]=value
            with self.assertRaises(ValueError):summarize('\n'.join('MESH_WELD_CHANNEL_NATIVE '+json.dumps(r)for r in rows))
