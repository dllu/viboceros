"""Frozen component-selection experiments and honest command qualification."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from . import mesh_component_unweld_cases as sdk
from . import mesh_component_unweld_command_cases as commands
from . import mesh_component_unweld_primed_cases as primed
from . import mesh_component_unweld_verified_cases as verified
from . import mesh_component_unweld_holdout_cases as holdout
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class MeshComponentUnweld(unittest.TestCase):
    def test_frozen_recipes_channels_and_original_producers(self):
        for suffix,module in [('channels',sdk),('commands',commands),('primed',primed),('verified',verified),('holdout',holdout)]:
            family='mesh_component_unweld_'+suffix
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_text())
            self.assertEqual(query,module.request());validate_operations(query)
            native=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id']for r in native['results']],list(module.CASES))
            for row in native['results']:
                v=row['value'];case=module.CASES[row['id']]
                self.assertEqual(v['input'],case)
                recipe=v.get('recipe_mesh',v['before'])
                self.assertEqual(recipe['vertices'],case['source']['vertices'])
                self.assertEqual(recipe['faces'],case['source']['faces'])
                self.assertEqual(recipe['normals'],case.get('stored_normals',[]))
                self.assertIs(v['before']['valid'],True);self.assertIs(v['after']['valid'],True)
                if 'transparency'in case:self.assertEqual([c[3]for c in recipe['colors']],case['transparency'])
                if 'recipe_mesh'in v:
                    for field in ['vertices','faces','uv','colors','ngons']:
                        self.assertEqual(v['before'][field],recipe[field])
                    self.assertEqual(v['after'],v['stages']['document_after'])
                    self.assertEqual(v['after_copy'],v['after'])
            for name,digest in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)

    def test_private_bounded_capture_and_fixed_selection_recipes(self):
        for make in [sdk.request,verified.request,holdout.request]:
            for extra in [dict(case=[]),dict(indices=[1]),dict(modify_normals=False),dict(script='arbitrary')]:
                query=make();query['operations'][0].update(extra)
                with self.assertRaises(ValueError):validate_operations(query)
            with patch.dict('os.environ',{'DISPLAY':':997','VIBOCEROS_ORACLE_HEADLESS':':997'}),patch('subprocess.Popen') as launch:
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(make())
                client=OracleClient(settings_scheme='VibocerosOracleComponentUnweldValidation')
                for kind in ['boolean','repeat','duplicate','large','mixed']:
                    query=make()
                    if kind=='boolean':query['iterations']=True
                    elif kind=='repeat':query['iterations']=2
                    elif kind=='duplicate':query['operations'].append(query['operations'][0])
                    elif kind=='large':query['operations']*=4
                    else:
                        from .mesh_weld_channels_cases import request
                        query['operations'].append(request()['operations'][0])
                    with self.assertRaises(OracleProtocolError):client.run_rhino(query)
                launch.assert_not_called()

    def test_command_normal_setup_and_sdk_routes_are_distinct_evidence(self):
        from .report_mesh_component_unweld import route_diagnostics
        rows=route_diagnostics()
        self.assertEqual(len(rows['sdk_vertex_edge_pairs']),20)
        self.assertEqual(len(rows['unprimed_same_output_pairs']),24)
        self.assertEqual(len(rows['primed_distinct_normal_pairs']),6)
        self.assertFalse(rows['unprimed_normal_modes_qualified'])
        self.assertTrue(rows['direct_document_copies_exact'])

    def test_report_requires_every_case_command_and_explicit_index_limit(self):
        from .report_mesh_component_unweld import ROOT,summarize
        report=json.loads((ROOT/'docs/mesh-component-unweld-qualification.json').read_text())
        geometry=['COMPONENT_UNWELD_NATIVE '+json.dumps(r)for r in report['cases']]
        sdk_lines=['COMPONENT_UNWELD_SDK '+json.dumps(r)for r in report['sdk_diagnostics']]
        command_lines=[]
        for row in report['commands']:
            r=dict(row);strict=r['id'].startswith('verified_')
            if strict:r.pop('raw_order_qualified')
            command_lines.append(('MESH_SEAM_COMMAND 'if strict else 'MESH_SEAM_COMPARISON ')+json.dumps(r))
        self.assertEqual(summarize('\n'.join(geometry+sdk_lines),'\n'.join(command_lines)),report)
        self.assertEqual((report['compared_cases'],report['geometry_channels_and_sharing_qualified_cases'],report['raw_order_qualified_cases']),(72,72,64))
        self.assertEqual((report['command_cases'],report['sdk_compared_cases'],report['sdk_raw_order_qualified_cases']),(106,48,28))
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for geos,sdks,cmds in [(geometry[:-1],sdk_lines,command_lines),(geometry+geometry[:1],sdk_lines,command_lines),
                               (geometry,sdk_lines[:-1],command_lines),(geometry,sdk_lines,command_lines[:-1]),
                               (geometry,sdk_lines,command_lines+command_lines[:1])]:
            with self.assertRaises(ValueError):summarize('\n'.join(geos+sdks),'\n'.join(cmds))
        for field,value in [('qualified',1),('raw_order_qualified',1),('vertices',True),('differences',['unmeasured'])]:
            rows=copy.deepcopy(report['cases']);rows[0][field]=value
            with self.assertRaises(ValueError):summarize('\n'.join('COMPONENT_UNWELD_NATIVE '+json.dumps(r)for r in rows)+'\n'+'\n'.join(sdk_lines),'\n'.join(command_lines))
