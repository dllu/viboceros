"""Closed-chart command evidence and public SDK observations remain reproducible."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_soft_edit_surface_seams import request as seams
from .generate_soft_edit_surface_closed_sequence import request as sequences
from .generate_soft_edit_surface_sdk import request as sdk
from .generate_soft_edit_surface_repeat_sdk import request as repeat_sdk
from .soft_edit_surface_seams_probe import validate
from .structural_sdk import stage_helpers, validate_operations
HERE = Path(__file__).resolve().parent


def capture(stem):
    return json.loads((HERE/'observations'/(stem+'.json')).read_text())


class ClosedSurfaceSoftEditTests(TestCase):
    def test_producer_closure_requests_and_fixture_bytes(self):
        digest = lambda b: hashlib.sha256(b).hexdigest()
        for stem, count, factory in [('soft_edit_surface_seams',55,seams),('soft_edit_surface_closed_sequence',12,sequences),('soft_edit_surface_sdk',7,sdk),('soft_edit_surface_repeat_sdk',4,repeat_sdk)]:
            path = HERE/'fixtures'/(stem+'.json')
            request = json.loads(path.read_text())
            response = capture(stem)
            self.assertEqual(len(response['results']),count)
            self.assertEqual([o['id'] for o in request['operations']],[o['id'] for o in response['results']])
            validate_operations(request)
            provenance = response['provenance']
            self.assertEqual(digest(path.read_bytes()),provenance['request_sha256'])
            for name, expected in provenance['sources'].items():
                actual = (HERE/name).read_bytes()
                if digest(actual) != expected:
                    actual = gzip.decompress((HERE/'observations/source_snapshots'/(expected+'.gz')).read_bytes())
                self.assertEqual(digest(actual),expected,name)
            for name, expected in provenance['fixtures'].items():
                # SDK jobs only read the pre-existing shape fixtures.
                folder = 'soft_edit_surface_closed_sequence' if stem=='soft_edit_surface_repeat_sdk' else 'soft_edit_surface_shapes' if stem=='soft_edit_surface_sdk' else stem
                actual = HERE/'fixtures'/folder/Path(name).name
                self.assertEqual(digest(actual.read_bytes()),expected,name)
            generated = factory()
            for a,b in zip(generated['operations'],request['operations']):
                self.assertEqual(Path(a.pop('artifact_path')).name,Path(b.pop('artifact_path')).name)
            self.assertEqual(generated,request)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(factory(),HERE,Path(folder))
                names = {p.name for p in Path(folder).iterdir()}
                self.assertIn(stem+'_probe.py',names)
                for path in Path(folder).iterdir():
                    self.assertEqual(path.read_bytes(),(HERE/path.name).read_bytes())

    def test_terminal_command_events_and_retained_boundaries(self):
        for stem in ('soft_edit_surface_seams','soft_edit_surface_closed_sequence'):
            for row in capture(stem)['results']:
                self.assertNotIn('error',row)
                value = row['value']
                events = [e['name'] for e in value['events']]
                self.assertEqual(events,['SelID','SoftEditSrf'] if value['recipe'].get('preselect') is False else ['SoftEditSrf'])
                self.assertNotIn('Unknown command',value['history'])
                self.assertEqual(value['model_tolerance'],1e-6)
                self.assertEqual(len(value['accepted_validity']),len(value['after']))
                for obj in value['accepted_validity']:
                    self.assertEqual(len(obj['seam_gaps']),2)
                    for trim in obj['trims']:
                        self.assertEqual(len(trim['uv']),17)
                        self.assertEqual(len(trim['surface_points']),17)
                        self.assertTrue(trim['edge_points'] is None or len(trim['edge_points'])==17)
        rows={r['id']:r['value'] for r in capture('soft_edit_surface_seams')['results']}
        self.assertGreater(rows['torus_start']['accepted_validity'][0]['seam_gaps'][1],1e-5)
        self.assertTrue(rows['torus_start']['accepted_validity'][0]['valid'])
        self.assertGreater(max(rows['torus_start']['accepted_validity'][0]['edge_tolerances']),1e-5)
        self.assertEqual(len(rows['periodic_copy']['after']),1)
        self.assertIsNone(rows['periodic_copy']['undo'])
        sequence={r['id']:r['value'] for r in capture('soft_edit_surface_closed_sequence')['results']}
        self.assertEqual(len(sequence['periodic_copy']['after']),1)
        self.assertIsNone(sequence['periodic_copy']['undo'])
        self.assertEqual(len(sequence['cylinder_copy']['after']),2)
        self.assertEqual(len(sequence['torus_copy']['after']),4)

    def test_public_surface_method_matches_command_tensor(self):
        command={r['id']:r['value'] for r in capture('soft_edit_surface_shapes')['results']}
        for row in capture('soft_edit_surface_sdk')['results']:
            self.assertNotIn('error',row)
            sdk = row['value']
            expected=command[row['id']]['after'][0]['geometry']['surfaces'][0]
            self.assertEqual(sdk['result'],expected)
        periodic=next(r['value'] for r in capture('soft_edit_surface_sdk')['results'] if r['id']=='periodic_start')
        self.assertEqual(periodic['raw_parameter'][1],periodic['source']['domain_v'][1])
        self.assertTrue(periodic['result_valid'])
        self.assertFalse(periodic['brep_valid'])

    def test_repeated_sdk_transitions_match_the_command_tensor(self):
        commands={r['id']:r['value'] for r in capture('soft_edit_surface_closed_sequence')['results']}
        for row in capture('soft_edit_surface_repeat_sdk')['results']:
            self.assertNotIn('error',row)
            variant=next(v for v in row['value']['variants'] if v['mode']=='updated')
            self.assertEqual(len(variant['steps']),3)
            expected=commands[row['id']]['after'][0]['geometry']['surfaces'][0]
            self.assertEqual(variant['steps'][-1]['result'],expected)
            for a,b in zip(variant['steps'],variant['steps'][1:]):
                self.assertEqual(a['result'],b['source'])

    def test_invalid_chart_recipes_fail_without_native_execution(self):
        base=seams()['operations'][0]
        for patch in [dict(skew_v=1),dict(distance=[0.,1.]),dict(extra=True)]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base),**patch))
