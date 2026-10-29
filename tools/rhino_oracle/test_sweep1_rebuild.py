"""Rebuilt sections: frozen public outputs, overload routing and standalone staging."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock
from .generate_sweep1_rebuild import request
from .generate_sweep1_rebuild_holdouts import query as holdouts
from . import sweep1_probe

HERE=Path(__file__).resolve().parent


class SweepRebuildTests(unittest.TestCase):
    def test_requests_outputs_and_producers_are_frozen(self):
        commands=request(command=True)
        commands['operations']=[op for op in commands['operations'] if op['rebuild_point_count'] in [0,3,5,10]]
        for family,expected in [('sweep1_rebuild_sdk',request()),('sweep1_rebuild_commands',commands),('sweep1_rebuild_holdouts',holdouts())]:
            with self.subTest(family=family):
                fixture=HERE/('fixtures/'+family+'.json')
                observed=json.loads((HERE/('observations/'+family+'.json')).read_text())
                self.assertEqual(json.loads(fixture.read_text()),expected)
                self.assertEqual(observed['engine'],'rhino')
                self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),observed['provenance']['request_sha256'])
                self.assertEqual([row['id']for row in observed['results']],[op['id']for op in expected['operations']])
                self.assertIn('sweep1_probe.py',observed['provenance']['sources'])
                for digest in observed['provenance']['sources'].values():
                    data=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(data).hexdigest(),digest)
                for row in observed['results']:
                    self.assertEqual(len(row['value']),1)
                    self.assertTrue(all(p['weight']>0 for p in row['value'][0]['definition']['control_points']))

    def test_sdk_overload_receives_rebuild_mode_and_count_without_losing_ownership(self):
        for count in [0,1,2,5,512]:
            with self.subTest(count=count):
                curves=[SimpleNamespace(Dispose=Mock())for _ in range(2)]
                brep=SimpleNamespace(Dispose=Mock())
                create=Mock(return_value=[brep])
                geometry=SimpleNamespace(Brep=SimpleNamespace(CreateFromSweep=create),Point3d=SimpleNamespace(Unset=None),Vector3d=SimpleNamespace(Unset=None),SweepFrame=object(),SweepBlend=object(),SweepMiter=object(),SweepRebuild=object())
                host=dict(Rhino=SimpleNamespace(Geometry=geometry),System=SimpleNamespace(Enum=SimpleNamespace(ToObject=lambda kind,value:value)))
                host.update(_join_close_input=Mock(side_effect=curves),_sweep1_roadlike_axis=lambda *args:None,_measure=lambda n,build:(build(),0),_sweep1_record=lambda *args:[{'recorded':True}])
                result,_=sweep1_probe.run(host,dict(rail={},sections=[{}],rebuild_point_count=count),1,dict(absolute=1e-7))
                self.assertEqual(result,[{'recorded':True}])
                self.assertEqual(create.call_args.args[-3:],(int(count>0),count,0.))
                for item in [*curves,brep]:item.Dispose.assert_called_once_with()

    def test_invalid_counts_fail_before_accessing_a_rhino_host(self):
        cases=[dict(rebuild_point_count=n)for n in [-1,513,True,5.,'5',None]]
        cases += [dict(command=True,rebuild_point_count=n)for n in [1,2]]
        for operation in cases:
            before=copy.deepcopy(operation)
            with self.subTest(operation=operation),self.assertRaises(ValueError):
                sweep1_probe.run({},operation,1,{})
            self.assertEqual(operation,before)
        for count in [1,2]:sweep1_probe.validate(dict(rebuild_point_count=count))

    def test_real_staging_imports_companion_in_an_isolated_python_process(self):
        from .structural_sdk import stage_helpers,validate_operations
        payload=request()
        validate_operations(payload)
        with tempfile.TemporaryDirectory() as folder:
            job=Path(folder)
            stage_helpers(payload,HERE,job)
            self.assertEqual({p.name for p in job.iterdir()},{'sweep1_probe.py'})
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import sweep1_probe;sweep1_probe.validate({});assert "_Simplify=_None" in sweep1_probe.macro({},["rail","profile"])',str(job)],check=True,capture_output=True,text=True)

    def test_macro_selects_rebuild_count_and_retains_all_other_script_options(self):
        macro=sweep1_probe.macro(dict(rebuild_point_count=5,refit_rail=True,blend=1),['rail','profile'])
        self.assertIn('_Simplify=_Rebuild _RebuildCount=5',macro)
        self.assertIn('_ShapeBlending=_Global _RefitRail=_Yes',macro)
        for count in [1,2]:
            with self.assertRaises(ValueError):sweep1_probe.macro(dict(rebuild_point_count=count),['rail','profile'])
        self.assertIn('_Simplify=_None',sweep1_probe.macro(dict(rebuild_point_count=0),['rail','profile']))

    def test_validation_binds_complete_images_and_historical_sources(self):
        root=HERE.parent.parent
        record=json.loads((root/'docs/sweep1/rebuild-validation.json').read_text())
        self.assertTrue(record['cross_section_rebuild_implemented'])
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        self.assertEqual(record['construction_tolerance'],1e-7)
        self.assertEqual(record['comparison_epsilon'],1e-7)
        for key in ['portable_images','native_images']:
            rows=record[key]
            self.assertEqual(len(rows),32)
            self.assertEqual(len({(r['family'],r['id'])for r in rows}),32)
            self.assertTrue(all(0<=r['complete_image_bound']<=1e-7 for r in rows))
        for check in record['checks'].values():self.assertEqual(check['exit_code'],0)
        self.assertGreater(verify_measurement(record, expand_sources=with_shared_sources), 0)


if __name__=='__main__':unittest.main()
