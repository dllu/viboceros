"""Authored inputs, frozen producers, rejected-point behavior and calibration scope."""
import base64,copy,hashlib,json,os,math
from pathlib import Path
import tempfile,unittest
from unittest import mock
from .client import OracleClient,OracleProtocolError
from .mesh_insert_evidence import HERE,FAMILIES,record,complete,classify_paths,INPUT_FAILURES
from .generate_insert_point_mesh_repeats import request as repeats
from .generate_mesh_insert_paths import request as paths
from .generate_mesh_insert_degenerate import request as sources
from .generate_mesh_insert_disconnected import request as islands
from .generate_mesh_insert_validation import request as validity
from .generate_mesh_insert_conversion import request as conversion
from .structural_sdk import validate_operations,stage_helpers
from .test_merge_faces_command import frozen
BUILDERS={'insert_point_mesh_repeats':repeats,'insert_point_mesh_repeats_second':repeats,
          'mesh_insert_paths':paths,'mesh_insert_degenerate':sources,'mesh_insert_disconnected':islands,
          'mesh_insert_validation':validity,'mesh_insert_conversion':conversion}
class MeshInsertSourceTests(unittest.TestCase):
    def test_every_request_producer_and_saved_input_is_bound(self):
        for family,count in FAMILIES.items():
            q=BUILDERS[family]();validate_operations(q);r=record(family)
            fixture=HERE/'fixtures'/(family+'.json')
            self.assertEqual(json.loads(fixture.read_text()),q)
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            self.assertEqual(len(r['results']),count)
            self.assertEqual([o['id']for o in q['operations']],[o['id']for o in r['results']])
            for name in r['provenance']['sources']:frozen(r,name)
            if family not in ['mesh_insert_validation','mesh_insert_conversion']:
                for row in r['results']:
                    data=base64.b64decode(row['value']['source_3dm'],validate=True)
                    self.assertTrue(data.startswith(b'3D Geometry File Format'))
                    if family in ['mesh_insert_paths','mesh_insert_degenerate','mesh_insert_disconnected']:
                        self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),data)

    def test_authored_sources_are_independent_of_outputs(self):
        from .mesh_insert_degenerate_cases import CASES
        r=record('mesh_insert_degenerate');namespace={}
        # This producer's sole local dependency is the authored source helper.
        producer=frozen(r,'mesh_insert_degenerate_cases.py').decode().replace('if __package__:',"if True:").replace('from .subdivide_probe import source','from tools.rhino_oracle.subdivide_probe import source')
        exec(compile(producer,'bound flat-corner cases','exec'),namespace)
        self.assertEqual(namespace['CASES'],CASES)
        for row in r['results']:
            case=copy.deepcopy(row['value']['input']);case.pop('capture_id')
            self.assertEqual(case,CASES[row['id']])
            self.assertTrue(complete(row['value']))
            self.assertNotIn('Tessellation failure',row['value']['history'])
        self.assertEqual(CASES['flat-corner']['sources'][0]['faces'],[[0,1,2,3]])

    def test_incomplete_inputs_and_native_tessellation_failure_remain_excluded(self):
        result=classify_paths();self.assertEqual(len(result['diagnostic_inputs']),17)
        rows={r['id']:r['value']for r in record('mesh_insert_paths')['results']}
        for name in INPUT_FAILURES:
            v=rows[name];self.assertFalse(complete(v));self.assertFalse(v['success'])
            def mesh(state):return next(o['mesh']for o in state if o['source']==0)
            for field in ['vertices','faces']:self.assertEqual(mesh(v['before'])[field],mesh(v['after'])[field])
        self.assertIn('Tessellation failure',rows['warped-edge-1']['history'])
        self.assertTrue(complete(rows['warped-edge-1']))
        self.assertTrue(rows['warped-edge-1']['success'])

    def test_native_repeats_preserve_coordinates_cells_and_input_witnesses(self):
        old={r['id']:r['value']for family in ['insert_point_full','insert_point_regions','insert_point_spatial_layouts','insert_point_spatial_prefixes']for r in record(family)['results']}
        def geometry(v,phase):return next(o['mesh']for o in v[phase]if o['source']==0)
        def cycles(faces):return sorted(min(tuple(f[i:]+f[:i])for i in range(len(f)))for f in faces)
        for family in ['insert_point_mesh_repeats','insert_point_mesh_repeats_second']:
            for row in record(family)['results']:
                first=old[row['id']];second=row['value'];self.assertTrue(complete(second))
                for phase in ['before','after']:
                    a=geometry(first,phase);b=geometry(second,phase)
                    self.assertEqual(a['vertices'],b['vertices'])
                    self.assertEqual(cycles(a['faces']),cycles(b['faces']))
                for a,b in zip(first['input_trace'][0]['clicks'],second['input_trace'][0]['clicks']):
                    self.assertTrue(all(math.dist(x,y)<=1e-12 for x,y in zip(a['ray'],b['ray'])))

    def test_disconnected_native_picks_undo_and_return_to_last_accepted_island(self):
        expected={'islands-reject':9,'islands-reverse':9,'islands-return':10,'islands-undo':8}
        for row in record('mesh_insert_disconnected')['results']:
            v=row['value'];self.assertTrue(complete(v))
            mesh=next(o['mesh']for o in v['after']if o['source']==0)
            self.assertEqual(len(mesh['vertices']),expected[row['id']])
            self.assertEqual(v['success'],row['id']!='islands-undo')

    def test_public_api_diagnostics_are_output_derived_and_not_command_inputs(self):
        original={r['id']:next(o['mesh']for o in r['value']['after']if o['source']==0)for r in record('mesh_insert_paths')['results']}
        assets=json.loads((HERE/'fixtures/mesh_insert_validation_assets.json').read_text())
        self.assertEqual(set(assets),{r['id']for r in record('mesh_insert_validation')['results']})
        for name,mesh in assets.items():self.assertEqual(mesh,original[name])
        for family in ['mesh_insert_validation','mesh_insert_conversion']:
            r=record(family)
            self.assertEqual(frozen(r,'fixtures/mesh_insert_validation_assets.json'),(HERE/'fixtures/mesh_insert_validation_assets.json').read_bytes())
            for row in r['results']:self.assertTrue(row['value']['valid'])

    def test_source_exports_cannot_overwrite_inputs(self):
        from . import mesh_insert_inputs as export
        with tempfile.TemporaryDirectory()as directory,mock.patch.object(export,'HERE',Path(directory)):
            root=Path(directory);(root/'observations').mkdir();r=record('mesh_insert_degenerate')
            (root/'observations/mesh_insert_degenerate.json').write_text(json.dumps(r))
            export.save_sources('mesh_insert_degenerate');export.save_sources('mesh_insert_degenerate')
            p=root/'fixtures/mesh_insert_degenerate/flat-corner.3dm';p.write_bytes(b'owned changed input')
            with self.assertRaisesRegex(ValueError,'existing native source differs'):export.save_sources('mesh_insert_degenerate')
            self.assertEqual(p.read_bytes(),b'owned changed input')

    def test_private_capture_guards_reject_mixed_repeated_public_and_oversized_requests(self):
        for make in [paths,sources,islands,validity,conversion]:
            q=make()
            for scheme,query in [(None,q),('VibocerosOracleMeshSourceTest',dict(q,iterations=True)),('VibocerosOracleMeshSourceTest',dict(q,iterations=2)),
                    ('VibocerosOracleMeshSourceTest',dict(q,operations=q['operations']*65)),
                    ('VibocerosOracleMeshSourceTest',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
                with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
                      mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),
                      mock.patch('tools.rhino_oracle.client._run_logged')as launch,
                      self.assertRaisesRegex(OracleProtocolError,'private dedicated')):
                    OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(query))
                launch.assert_not_called()
            with tempfile.TemporaryDirectory()as directory:
                stage_helpers(q,HERE,Path(directory))
                for name in ['join_probe.py','merge_edges_probe.py']:self.assertTrue((Path(directory)/name).is_file())
                if make in [validity,conversion]:self.assertTrue((Path(directory)/'fixtures/mesh_insert_validation_assets.json').is_file())

    def test_current_receipt_binds_sources_and_retained_families(self):
        from .qualification_sources import MESH_INSERT_SOURCE_MODULES,with_shared_sources
        r=json.loads((HERE.parent.parent/'docs/geometry/mesh-insert-sources-validation.json').read_text())
        self.assertEqual(r['qualified_new_workflows'],10);self.assertEqual(r['coordinate_epsilon'],1e-6)
        for name in MESH_INSERT_SOURCE_MODULES:self.assertTrue(set(MESH_INSERT_SOURCE_MODULES)<=set(with_shared_sources([name])))
        for name,digest in r['sources'].items():self.assertEqual(hashlib.sha256((HERE.parent.parent/name).read_bytes()).hexdigest(),digest,name)
        for family in FAMILIES:
            for folder,digest in r['families'][family].items():self.assertEqual(hashlib.sha256((HERE/folder/(family+'.json')).read_bytes()).hexdigest(),digest)
