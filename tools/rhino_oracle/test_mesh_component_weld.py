"""Native channel observations, owned capture boundaries and strict reporting."""
from tools.verify_provenance import reconstruct_measurement
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from . import mesh_component_weld_cases as base
from . import mesh_component_weld_followup_cases as followup
from . import mesh_component_weld_holdout_cases as holdout
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class MeshComponentWeld(unittest.TestCase):
    def test_frozen_recipes_and_native_source_snapshots(self):
        for suffix,module in [('commands',base),('followup',followup),('holdout',holdout)]:
            family='mesh_component_weld_'+suffix
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_text())
            self.assertEqual(query,module.request());validate_operations(query)
            native=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id']for r in native['results']],list(module.CASES))
            for row in native['results']:
                value=row['value'];case=module.CASES[row['id']]
                self.assertEqual(value['input'],case)
                recipe=value.get('recipe_mesh',value['before'])
                self.assertEqual(recipe['vertices'],case['source']['vertices'])
                self.assertEqual(recipe['faces'],case['source']['faces'])
                self.assertEqual(recipe['normals'],case.get('stored_normals',[]))
                if value['invoked']:
                    self.assertTrue(value['before']['valid'])
                    self.assertEqual(value['after'],value['after_copy'])
                    self.assertEqual(value['before'],value['stages']['document_before'])
                    self.assertEqual(value['after'],value['stages']['document_after'])
                elif case['kind']!='whole':self.assertEqual(value['after'],value['before'])
            for name,digest in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)

    def test_owned_bounded_capture_and_fixed_recipes(self):
        for make in [base.request,followup.request,holdout.request]:
            for extra in [dict(case=[]),dict(indices=[0]),dict(script='arbitrary')]:
                query=make();query['operations'][0].update(extra)
                with self.assertRaises(ValueError):validate_operations(query)
            with patch.dict('os.environ',{'DISPLAY':':997','VIBOCEROS_ORACLE_HEADLESS':':997'}),patch('subprocess.Popen') as launch:
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(make())
                client=OracleClient(settings_scheme='VibocerosOracleComponentWeldValidation')
                for kind in ['boolean','repeat','duplicate','large','mixed']:
                    query=make()
                    if kind=='boolean':query['iterations']=True
                    elif kind=='repeat':query['iterations']=2
                    elif kind=='duplicate':query['operations'].append(query['operations'][0])
                    elif kind=='large':query['operations']*=4
                    else:
                        from .mesh_unweld_channels_cases import request
                        query['operations'].append(request()['operations'][0])
                    with self.assertRaises(OracleProtocolError):client.run_rhino(query)
                launch.assert_not_called()

    def test_reports_keep_all_cases_commands_and_explicit_limits(self):
        from .report_mesh_component_weld import ROOT,summarize
        report=json.loads((ROOT/'docs/mesh-component-weld-qualification.json').read_text())
        def lines(prefix,rows):return '\n'.join(prefix+' '+json.dumps(r)for r in rows)
        geometry=lines('MESH_COMPONENT_WELD_NATIVE',report['cases'])
        recipes=lines('MESH_COMPONENT_WELD_RECIPE',report['generated_normals'])
        commands=lines('MESH_COMPONENT_WELD_COMMAND',report['commands'])
        self.assertEqual(reconstruct_measurement(report, summarize, geometry+'\n'+recipes,commands),report)
        self.assertEqual((report['compared_cases'],report['geometry_and_raw_sharing_qualified_cases'],report['channels_and_raw_order_qualified_cases']),(56,56,52))
        self.assertEqual((report['generated_normal_cases'],report['command_cases'],report['qualified_command_cases']),(6,66,61))
        self.assertEqual(len(report['invalid_source_diagnostics']),4)
        self.assertEqual(len(report['native_invalid_output_cases']),2)
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for geos,cmds in [(geometry,commands),(geometry+'\n'+recipes,commands+'\n'+commands.splitlines()[0]),(geometry+'\n'+recipes,'\n'.join(commands.splitlines()[:-1]))]:
            with self.assertRaises(ValueError):summarize(geos,cmds)
        for field,value in [('qualified',1),('topology_qualified',1),('vertices',True),('differences',['unmeasured'])]:
            rows=copy.deepcopy(report['cases']);rows[0][field]=value
            with self.assertRaises(ValueError):summarize(lines('MESH_COMPONENT_WELD_NATIVE',rows)+'\n'+recipes,commands)
