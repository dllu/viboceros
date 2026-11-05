"""Bounded producer admission and immutable native selection-only evidence."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .type_selection_cases import cases,empty_cases
from .subd_edge_analysis_replay import HERE,read
from .structural_sdk import stage_helpers,validate_operations


class TypeSelectionTests(unittest.TestCase):
    def test_fixed_requests_and_frozen_producers(self):
        for family,recipes in [('type_selection',cases()),('type_selection_clean',cases()),('type_selection_empty',empty_cases())]:
            self._frozen(family,recipes)

    def _frozen(self,family,recipes):
        path=HERE/('fixtures/'+family+'.json');request=json.loads(path.read_bytes())
        validate_operations(request)
        self.assertEqual(request['operations'],recipes)
        native=read(family)
        self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual([r['id'] for r in native['results']],[op['id'] for op in recipes])
        for name,digest in native['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)

    def test_native_selection_never_edits_sources_or_consumes_redo(self):
        for family in ('type_selection_clean','type_selection_empty'):
            for row in read(family)['results']:
                self._unchanged(family,row)

    def _unchanged(self,family,row):
        v=row['value']
        self.assertTrue(v['redone'],row['id'])
        for phase in ('before','after','repeat','previous','redo'):
            self.assertTrue(v[phase]['geometry_unchanged'],(row['id'],phase))
            self.assertEqual(v[phase]['points'],[[90.,91.,92.]] if phase=='redo' else [])
        self.assertEqual(v['after'],v['repeat'],row['id'])
        if family=='type_selection_clean':self.assertEqual(v['after'],v['previous'],row['id'])
        self.assertIn('Redoing Point',v['history'])

    def test_admission_and_transitive_staging(self):
        for op in cases()+empty_cases():
            for change in (dict(command='Delete'),dict(variant='arbitrary'),dict(extra=True),dict(name=['Bolt A']),dict(preselected=1)):
                with self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op),**change)]))
        request=dict(protocol_version=1,iterations=1,operations=cases()+empty_cases())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request,HERE,Path(folder))
            for module in ('type_selection_probe','type_selection_cases','type_selection_scene','join_probe','merge_edges_probe'):
                self.assertTrue((Path(folder)/(module+'.py')).is_file())
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import type_selection_probe',folder],check=True,capture_output=True)

    def test_earlier_guarded_capture_preserves_first_command_evidence(self):
        for a,b in zip(read('type_selection')['results'],read('type_selection_clean')['results'],strict=True):
            self.assertEqual(a['id'],b['id'])
            for phase in ('before','after'):self.assertEqual(a['value'][phase],b['value'][phase],a['id'])


if __name__=='__main__':
    unittest.main()
