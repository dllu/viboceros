"""Frozen public evidence and complete qualification of the normal-policy option."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from . import mesh_unweld_channels_cases as cases
from .client import OracleClient,OracleProtocolError
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


class MeshUnweldChannels(unittest.TestCase):
    def test_frozen_inputs_producers_and_stored_normal_channels(self):
        query=HERE/'fixtures/mesh_unweld_channels.json'
        self.assertEqual(json.loads(query.read_text()),cases.request());validate_operations(cases.request())
        native=json.loads((HERE/'observations/mesh_unweld_channels.json').read_text())
        self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
        self.assertEqual([r['id'] for r in native['results']],list(cases.CASES))
        for row in native['results']:
            v=row['value'];self.assertEqual(v['input'],cases.CASES[row['id']])
            self.assertIs(v['before']['valid'],True);self.assertIs(v['after']['valid'],True)
            self.assertEqual(v['before']['vertices'],v['input']['source']['vertices'])
            self.assertEqual(v['before']['faces'],v['input']['source']['faces'])
            self.assertEqual(v['before']['uv'],[[i,i*i]for i in range(len(v['before']['vertices']))])
            if 'transparency' in v['input']:
                self.assertEqual(v['before']['colors'],[[(17*i)%256,(31*i)%256,(47*i)%256,a]for i,a in enumerate(v['input']['transparency'])])
            if 'stored_normals' in v['input']:self.assertEqual(v['before']['normals'],v['input']['stored_normals'])
            else:self.assertEqual(v['before']['normals'],[]);self.assertEqual(v['after']['normals'],[])
        for name,digest in native['provenance']['sources'].items():
            data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)

    def test_capture_rejects_mutated_unowned_and_repeated_recipes(self):
        for extra in [dict(id='arbitrary'),dict(case=[]),dict(modify_normals=False),dict(script='arbitrary')]:
            query=cases.request();query['operations'][0].update(extra)
            with self.assertRaises(ValueError):validate_operations(query)
        with patch.dict('os.environ',{'DISPLAY':':997','VIBOCEROS_ORACLE_HEADLESS':':997'}),patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(cases.request())
            client=OracleClient(settings_scheme='VibocerosOracleUnweldValidation')
            for kind in ['boolean','repeat','duplicate','large']:
                query=cases.request()
                if kind=='boolean':query['iterations']=True
                elif kind=='repeat':query['iterations']=2
                elif kind=='duplicate':query['operations'].append(query['operations'][0])
                else:query['operations']*=4
                with self.assertRaises(OracleProtocolError):client.run_rhino(query)
            launch.assert_not_called()

    def test_report_requires_every_geometry_and_command_comparison(self):
        from .report_mesh_unweld_channels import ROOT,summarize
        report=json.loads((ROOT/'docs/mesh-unweld-channels-qualification.json').read_text())
        geometry=['MESH_UNWELD_CHANNEL_NATIVE '+json.dumps(r) for r in report['cases']]
        commands=['MESH_SEAM_COMMAND '+json.dumps(r) for r in report['commands']]
        self.assertEqual(summarize('\n'.join(geometry),'\n'.join(commands)),report)
        self.assertEqual((report['compared_cases'],report['qualified_cases'],report['command_cases']),(18,18,27))
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        for rows,cmds in [(geometry[:-1],commands),(geometry+geometry[:1],commands),(geometry,commands[:-1]),(geometry,commands+commands[:1])]:
            with self.assertRaises(ValueError):summarize('\n'.join(rows),'\n'.join(cmds))
        for field,value in [('qualified',1),('vertices',True),('differences',['normals'])]:
            rows=copy.deepcopy(report['cases']);rows[0][field]=value
            with self.assertRaises(ValueError):summarize('\n'.join('MESH_UNWELD_CHANNEL_NATIVE '+json.dumps(r)for r in rows),'\n'.join(commands))
