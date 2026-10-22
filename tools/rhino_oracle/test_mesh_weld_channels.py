"""Fixed producer admission and complete, explicit mesh welding comparisons."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from . import mesh_weld_channels_cases as cases
from . import mesh_weld_channels_stress_cases as stress
from . import mesh_weld_channels_trace_cases as trace
from . import mesh_weld_channels_options_cases as options
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class MeshWeldChannels(unittest.TestCase):
    def test_native_requests_inputs_and_frozen_producers(self):
        counts=[]
        for suffix,make,scenes in [('',cases.request,cases.CASES),('_followup',cases.followup_request,cases.FOLLOWUP),
                                   ('_prefixes',cases.prefixes_request,cases.PREFIXES),('_pairs',cases.pairs_request,cases.PAIRS),('_stress',stress.request,stress.CASES),('_trace',trace.request,trace.CASES),('_options',options.request,options.CASES)]:
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
                expected=scenes[row['id']]['source']['faces']
                if scenes[row['id']].get('late_invalid_face'):
                    n=len(scenes[row['id']]['source']['vertices']);expected=expected+[[n+2,n+3,n+4]]
                self.assertEqual(row['value']['before']['faces'],expected)
            for name,digest in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            counts.append(len(native['results']))
        self.assertEqual(counts,[18,13,10,15,29,8,20])

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
        self.assertEqual((report['compared_cases'],report['qualified_cases'],len(report['diagnostic_cases'])),(96,71,17))
        self.assertEqual(len(report['precompaction_diagnostics']),8)
        self.assertTrue(all(r['fields_exact'] for r in report['precompaction_diagnostics']))
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for bad in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(bad))
        for field,value in [('qualified',1),('vertices',True),('faces',-1),('differences',['unmeasured'])]:
            rows=copy.deepcopy(report['cases']);rows[0][field]=value
            with self.assertRaises(ValueError):summarize('\n'.join('MESH_WELD_CHANNEL_NATIVE '+json.dumps(r)for r in rows))

    def test_precompaction_diagnostics_are_bound_to_valid_native_twins(self):
        from .report_mesh_weld_channels import trace_comparisons
        records=trace_comparisons()
        self.assertEqual({r['id'] for r in records},set(trace.TWINS))
        self.assertTrue(all(r['raw_vertices']>r['compact_vertices'] for r in records))

    def test_radial_cache_changes_do_not_change_native_weld_output(self):
        from .report_mesh_weld_channels import radial_comparisons
        rows=radial_comparisons()
        self.assertEqual({r['id']for r in rows},set(options.RADIAL_TWINS))
        self.assertTrue(all(r['fields_exact'] and r['changed_radial_vertices']>0 for r in rows))
        native=json.loads((HERE/'observations/mesh_weld_channels_options.json').read_text())
        alpha=[r['value']for r in native['results'] if 'transparency' in r['value']['input']]
        self.assertEqual(len(alpha),12)
        for v in alpha:
            self.assertEqual([c[3]for c in v['before']['colors']],v['input']['transparency'])
