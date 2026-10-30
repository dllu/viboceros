"""Independent authored inputs, source precision, Native lifecycle and guards."""
import base64,copy,gzip,hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest import mock
from .cap_regions_evidence import HERE,FAMILIES,record,audit
from .generate_mesh_cap_regions import request as regions
from .generate_mesh_cap_regions_groups import request as groups
from .generate_mesh_cap_regions_noop import request as noop
from .mesh_cap_regions_cases import CASES,validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers,validate_operations
BUILDERS=dict(zip(FAMILIES,[regions,groups,noop]))


def frozen(record,name):
    digest=record['provenance']['sources'][name]
    data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
    if hashlib.sha256(data).hexdigest()!=digest:raise ValueError('corrupt frozen producer')
    return data


class CapRegionTests(unittest.TestCase):
    def test_requests_archived_producers_and_exact_sources_are_bound(self):
        for family,count in FAMILIES.items():
            q=BUILDERS[family]();validate_operations(q);r=record(family)
            raw=(HERE/'fixtures'/(family+'.json')).read_bytes()
            self.assertEqual(json.loads(raw),q)
            self.assertEqual(hashlib.sha256(raw).hexdigest(),r['provenance']['request_sha256'])
            self.assertEqual(len(r['results']),count)
            self.assertEqual([op['id']for op in q['operations']],[row['id']for row in r['results']])
            for name in r['provenance']['sources']:frozen(r,name)
            namespace={};exec(compile(frozen(r,'mesh_cap_regions_cases.py'),'frozen authored caps','exec'),namespace)
            for row in r['results']:
                self.assertEqual(namespace['CASES'][row['id']],CASES[row['id']])
                self.assertEqual(row['value']['input'],CASES[row['id']])
                data=base64.b64decode(row['value']['source_3dm'],validate=True)
                self.assertTrue(data.startswith(b'3D Geometry File Format'))
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),data)
        self.assertEqual(audit()['cases'],22)
        self.assertTrue(audit()['input_coordinates_exact'])

    def test_narrow_inputs_do_not_collapse_to_float_positions(self):
        self.assertIn('UseDoublePrecisionVertices=True',frozen(record('mesh_cap_regions'),'crease_probe.py').decode())
        for family in FAMILIES:
            for row in record(family)['results']:
                for index,source in enumerate(row['value']['input']['sources']):
                    before=next(o['mesh']for o in row['value']['before']if o['source']==index)
                    self.assertTrue(before['double_precision']);self.assertTrue(before['valid'])
                    self.assertEqual(before['vertices'],source['vertices'])
        for name in ['narrow','narrow-large']:
            points=CASES[name]['sources'][0]['vertices']
            self.assertGreater(points[16][0],points[9][0])
            self.assertLess(points[16][0]-points[9][0],2e-5)

    def test_all_native_outputs_remain_including_non_solid_successes(self):
        self.assertEqual(audit()['native_non_solid_outputs'],['separated-large','narrow-large','separated-large-reversed','narrow-welded','narrow-copy'])
        for row in record('mesh_cap_regions')['results']:
            self.assertTrue(row['value']['success'])
            self.assertTrue(all(o['mesh']['valid']for o in row['value']['after']if'mesh'in o))
        r=record('mesh_cap_regions');self.assertEqual(len(r['results']),14)

    def test_native_copies_omit_groups_and_leave_source_memberships_intact(self):
        total=0
        for family in ['mesh_cap_regions','mesh_cap_regions_groups']:
            for row in record(family)['results']:
                v=row['value'];outputs=[o for o in v['after']if'mesh'in o and o['source']is None]
                for output in outputs:
                    total+=1;self.assertEqual(output['groups'],[]);self.assertFalse(output['selected'])
                for source in v['before']:
                    if source['source']is None:continue
                    after=next(o for o in v['after']if o['source']==source['source'])
                    self.assertEqual(after['groups'],source['groups'])
        self.assertEqual(total,audit()['copied_meshes'])
        self.assertGreater(total,8)

    def test_native_noop_undo_consumes_only_the_preceding_witness(self):
        for row in record('mesh_cap_regions_noop')['results']:
            v=row['value']
            self.assertEqual([o['mesh']for o in v['before']if'mesh'in o],[o['mesh']for o in v['after']if'mesh'in o])
            self.assertTrue(any(o.get('name')=='Undo witness'for o in v['before']))
            self.assertFalse(any(o.get('name')=='Undo witness'for o in v['undo']))
            self.assertTrue(any(o.get('name')=='Undo witness'for o in v['redo']))

    def test_invalid_recipes_and_requests_cannot_launch_the_host(self):
        for op in [{},dict(op='mesh_cap_regions',id='narrow',case='narrow',vertices=[]),dict(op='mesh_cap_regions',id='other',case='narrow')]:
            with self.assertRaises(ValueError):validate(op)
        q=regions()
        for scheme,request in [(None,q),('VibocerosOracleCapGuard',dict(q,iterations=True)),('VibocerosOracleCapGuard',dict(q,iterations=2)),
                ('VibocerosOracleCapGuard',dict(q,operations=q['operations']*65)),
                ('VibocerosOracleCapGuard',dict(q,operations=q['operations']+[dict(op='curve_length',id='other')]))]:
            with mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),mock.patch('tools.rhino_oracle.client._rhino_process_ids',return_value=set()),mock.patch('tools.rhino_oracle.client._run_logged')as launch,self.assertRaisesRegex(OracleProtocolError,'private dedicated'):
                OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(copy.deepcopy(request))
            launch.assert_not_called()

    def test_shared_staging_contains_every_scene_dependency(self):
        for builder in BUILDERS.values():
            with tempfile.TemporaryDirectory()as path:
                stage_helpers(builder(),HERE,Path(path))
                for name in ['mesh_cap_regions_probe','mesh_cap_regions_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe']:
                    self.assertTrue((Path(path)/(name+'.py')).is_file(),name)

    def test_current_receipt_binds_the_sources_and_all_native_families(self):
        from .qualification_sources import CAP_MESH_NATIVE_MODULES,with_shared_sources
        root=HERE.parent.parent
        receipt=json.loads((root/'docs/geometry/cap-mesh-native-validation.json').read_text())
        self.assertEqual(receipt['native_command_workflows'],22)
        self.assertEqual(receipt['coordinate_epsilon'],1e-6)
        for name in CAP_MESH_NATIVE_MODULES:
            self.assertTrue(set(CAP_MESH_NATIVE_MODULES)<=set(with_shared_sources([name])))
        for name,digest in receipt['source_hashes'].items():
            self.assertEqual(hashlib.sha256((root/name).read_bytes()).hexdigest(),digest,name)
        for family in FAMILIES:
            for folder,digest in receipt['families'][family].items():
                self.assertEqual(hashlib.sha256((HERE/folder/(family+'.json')).read_bytes()).hexdigest(),digest)

    def test_native_narrow_cap_retains_edges_below_the_model_tolerance(self):
        import math
        row=next(r for r in record('mesh_cap_regions')['results']if r['id']=='narrow')
        mesh=next(o['mesh']for o in row['value']['after']if o['source']==0)
        lengths=[math.dist(mesh['vertices'][f[i]],mesh['vertices'][f[(i+1)%len(f)]])for f in mesh['faces'][12:]for i in range(len(f))]
        self.assertGreater(min(lengths),0.)
        self.assertLess(min(lengths),1e-9)
