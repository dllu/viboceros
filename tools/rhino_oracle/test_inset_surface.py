"""Frozen planar/curved surface inset evidence and explicit macro qualifications."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_surface_probe import validate as sdk_validate
from .inset_surface_command import cases, validate as command_validate
from .inset_surface_controls import cases as controls, validate as controls_validate


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class SurfaceInsetTests(unittest.TestCase):
    def test_frozen_coverage_and_sources(self):
        for family, count in [('inset_surface_sdk',42), ('inset_surface_controls',16), ('inset_surface_command',36), ('inset_surface_ready',36)]:
            query = request()
            if family == 'inset_surface_controls': query['operations'] = controls()
            elif family != 'inset_surface_sdk': query['operations'] = cases(family == 'inset_surface_ready')
            path = HERE / ('fixtures/' + family + '.json')
            data = native(family)
            self.assertEqual(json.loads(path.read_bytes()),query)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in query['operations']])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                contents = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(contents).hexdigest(),sha)
            for row in data['results']: self.assertNotIn('error',row)

    def test_sdk_sources_immutable_and_curved_cases_retained(self):
        data = native('inset_surface_sdk')['results']
        for row in data:
            self.assertTrue(row['value']['source_unchanged'])
            self.assertTrue(row['value']['before']['valid'])
            self.assertTrue(row['value']['output']['valid'])
        tight = next(r['value']['output'] for r in data if r['id']=='warped_tight_crease')
        loose = next(r['value']['output'] for r in data if r['id']=='warped_loose_crease')
        self.assertEqual(len(tight['geometry']['faces']),126)
        self.assertEqual(len(loose['geometry']['faces']),5)

    def test_command_outputs_selection_and_actual_history(self):
        for family in ('inset_surface_command','inset_surface_ready'):
            for row in native(family)['results']:
                v = row['value']; self.assertEqual(v['results'],['Success'])
                self.assertEqual('Unknown command: _Loose=_No' in v['history'],family=='inset_surface_command')
                if family == 'inset_surface_ready': self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual(v['before'][0]['brep'],v['undo'][0]['brep'])
                self.assertEqual([{k:x for k,x in o.items() if k not in ('selected','selected_faces')} for o in v['after']],
                                 [{k:x for k,x in o.items() if k not in ('selected','selected_faces')} for o in v['redo']])
                self.assertTrue(all(not o['selected_faces'] for o in v['after_script']))
                if row['id'].endswith('_joined'):
                    self.assertEqual(len(v['after']),1)
                    self.assertEqual(v['after'][0]['source'],0)
                    self.assertEqual(v['after'][0]['groups'],[0])
                    self.assertEqual(v['after'][0]['selected_faces'],[])
                elif row['id'].endswith('_pieces'):
                    self.assertTrue(all(o['source'] is None and o['groups']==[] for o in v['after']))
                else:
                    self.assertEqual(v['after'][0]['brep'],v['before'][0]['brep'])
                    self.assertTrue(all('curve' in o and o['groups']==[] for o in v['after'][1:]))

    def test_unowned_and_malformed_recipes_are_rejected(self):
        for check, op in [(sdk_validate,request()['operations'][0]), (controls_validate,controls()[0]), (command_validate,cases()[0]), (command_validate,cases(True)[0])]:
            check(op)
            for changes in ({'id':'unowned'}, {'scene':'arbitrary'}, {'crease_corners':1}, {'extra':True}):
                with self.assertRaises(ValueError): check(dict(op,**changes))
        for changes in ({'distance':True}, {'distance':float('inf')}, {'distance':float('nan')}):
            with self.assertRaises(ValueError): sdk_validate(dict(request()['operations'][0],**changes))


if __name__ == '__main__': unittest.main()
