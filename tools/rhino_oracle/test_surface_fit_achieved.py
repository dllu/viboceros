"""Owned public SDK bridge schema, packaging, ABI declaration and failed capture."""
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import generate_surface_fit_achieved as generator
from . import surface_fit_achieved_probe as probe
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).parent


class SurfaceFitAchievedTests(unittest.TestCase):
    def test_request_reproduces_fixture_and_standalone_packaging(self):
        q=generator.request();before=copy.deepcopy(q);validate_operations(q)
        self.assertEqual(q,json.loads((HERE/'fixtures/surface_fit_achieved.json').read_bytes()))
        self.assertEqual(len(q['operations']),15)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,HERE,Path(directory))
            self.assertEqual(sorted(p.name for p in Path(directory).iterdir()),['surface_fit_achieved_probe.py','surface_fit_probe.py'])
            sys.path.insert(0,directory)
            try:
                spec=importlib.util.spec_from_file_location('standalone_achieved',Path(directory)/'surface_fit_achieved_probe.py')
                module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
                self.assertFalse(module.__package__);module.validate(q['operations'][0])
            finally:sys.path.remove(directory)
        self.assertEqual(q,before)
        op=q['operations'][0]
        for change in [dict(op='surface_fit'),dict(degree_u=True),dict(fit_tolerance=-1.),dict(extra=True)]:
            with self.assertRaises(ValueError):probe.validate(dict(op,**change))

    def test_symbol_is_generated_from_the_documented_public_cpp_signature(self):
        result=subprocess.run(['clang++','--target=x86_64-pc-windows-msvc','-S','-emit-llvm',
            str(HERE/'public_surface_fit_signature.cpp'),'-o','-'],capture_output=True,text=True,check=True)
        self.assertIn(probe.SYMBOL.decode(),result.stdout)

    def test_import_failure_retains_the_exact_producer_closure(self):
        failed=list((HERE/'observations/failures').glob('surface_fit_achieved-*.json.gz'))
        self.assertEqual(len(failed),1)
        capsule=json.loads(gzip.decompress(failed[0].read_bytes()))
        self.assertIn('Attempted relative import in non-package',capsule['error'])
        for sha in capsule['provenance']['sources'].values():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),sha)


if __name__=='__main__':unittest.main()
