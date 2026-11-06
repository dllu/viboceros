"""Native Plane recipes, immutable producers, document behavior and isolation."""
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import struct
import sys
import tempfile
import unittest
from unittest import mock
from . import plane_surface_cases as cases
from .client import OracleClient, OracleProtocolError
from .structural_sdk import PROBES, stage_helpers, validate_operations

HERE = Path(__file__).parent
FAMILIES = [('plane_primitive_pilot', 8), ('plane_surface_workflows', 29),
            ('plane_surface_audit', 10), ('plane_surface_holdouts', 12),
            ('plane_surface_qualified', 51), ('plane_surface_terminal', 51), ('plane_surface_guides', 5), ('plane_surface_vertical_audit', 7)]


def observed(name):
    path = HERE / ('observations/' + name + '.json')
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))


class PlaneSurfaceTests(unittest.TestCase):
    def test_qualification_requires_complete_executed_evidence_and_keeps_the_gate(self):
        from .report_plane_surface import ROOT, build
        proof = ROOT / 'docs/plane-surface-proof'
        record = json.loads((ROOT / 'docs/plane-surface-validation.json').read_text())
        self.assertRegex(record['implementation_revision'], r'\A[0-9a-f]{40}\Z')
        self.assertEqual(record, build(proof, record['implementation_revision']))
        self.assertEqual(record['model_epsilon'], 1e-6)
        self.assertEqual((record['qualified_workflows'], record['script_constructions'], record['archive_round_trips_per_build']), (63, 48, 53))
        self.assertFalse(record['full_native_parity'])
        with self.assertRaisesRegex(ValueError, 'full implementation revision'):
            build(proof, 'HEAD')
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / 'proof'
            shutil.copytree(proof, copied)
            path = copied / 'ui-portable.log.gz'
            text = gzip.decompress(path.read_bytes()).decode()
            lines = text.splitlines(keepends=True)
            index = next(i for i, line in enumerate(lines) if line.startswith('PLANE_UI '))
            missing = ''.join(lines[:index] + lines[index+1:])
            path.write_bytes(gzip.compress(missing.encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'incomplete CLI or UI execution'):
                build(copied, record['implementation_revision'])
            row = json.loads(lines[index].split(' ', 1)[1])
            row['maximum_coefficient_and_sample_error'] = 1.1e-6
            lines[index] = 'PLANE_UI ' + json.dumps(row) + '\n'
            path.write_bytes(gzip.compress(''.join(lines).encode(), mtime=0))
            with self.assertRaisesRegex(ValueError, 'geometry gate failed'):
                build(copied, record['implementation_revision'])

    def test_fixed_factories_and_every_frozen_producer(self):
        factories = dict(plane_surface_workflows=cases.request,
                         plane_surface_audit=lambda: cases.request(cases.AUDIT),
                         plane_surface_holdouts=lambda: cases.request(cases.HOLDOUTS),
                         plane_surface_qualified=cases.qualified_request,
                         plane_surface_terminal=cases.qualified_request,
                         plane_surface_guides=lambda:cases.request(list(cases.GUIDE_CASES)),
                         plane_surface_vertical_audit=lambda:cases.request(cases.VERTICAL_AUDIT))
        for family, count in FAMILIES:
            raw = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, native = json.loads(raw), observed(family)
            if family in factories:
                self.assertEqual(query, factories[family]())
                for op in query['operations']: cases.validate(op)
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in query['operations']])
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(raw).hexdigest())
            for digest in native['provenance']['sources'].values():
                frozen = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), digest)

    def test_native_inputs_are_prescribed_and_not_derived_from_outputs(self):
        for row in observed('plane_surface_terminal')['results'] + observed('plane_surface_guides')['results'] + observed('plane_surface_vertical_audit')['results']:
            case = cases.recipe(row['id'])
            tokens = []
            for a in case['actions']:
                token = '_' + a if isinstance(a, str) else 'w' + ','.join('%.17g' % n for n in a) if isinstance(a, list) else '%.17g' % a
                tokens.append(token)
                if a == 'AroundCurve' and not case.get('skip_guide_pick'): tokens.extend(["'_-SelID", '@guide'])
            macro = re.sub(r"'_-SelID [0-9a-fA-F-]{36}", "'_-SelID @guide", row['value']['macro'])
            def canonical(word):
                if word.startswith('w'):
                    return tuple(struct.pack('!d', float(x)) for x in word[1:].split(','))
                return word
            self.assertEqual([canonical(w) for w in macro.split()], [canonical(w) for w in ('_Plane ' + ' '.join(tokens)).split()])

    def test_native_geometry_selection_attributes_history_and_model_tolerance(self):
        constructed = controls = recoveries = 0
        for row in observed('plane_surface_terminal')['results'] + observed('plane_surface_guides')['results'] + observed('plane_surface_vertical_audit')['results']:
            v = row['value']
            self.assertTrue(v['returned_idle'])
            self.assertEqual(v['model_tolerance'], cases.qualified_request()['tolerance'])
            cross = lambda a,b: [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
            unit = lambda a: [n/math.hypot(*a) for n in a]
            origin, x, y = v['construction_plane']; x = unit(x); y = unit(cross(unit(cross(x,y)),x))
            for actual,expected in zip(v['cplane_after'],[origin,x,y]):
                for a,b in zip(actual,expected): self.assertLessEqual(abs(a-b),1e-14)
            terminal = copy.deepcopy(v['created'])
            if 'AroundCurve' in cases.recipe(row['id'])['actions']:
                for o in terminal:
                    if o['guide'] and o['selected'] == 1: o['selected'] = 0
            self.assertEqual(terminal, v['after'])
            self.assertEqual(v['after'], v['redo'])
            events = [e for e in v['events'] if e['name'] == 'Plane']
            self.assertEqual(len(events), 1)
            self.assertEqual(events[0]['objects'], v['created'])
            released = copy.deepcopy(v['before'])
            if 'AroundCurve' in cases.recipe(row['id'])['actions'] and not any(o['guide'] and o['selected'] == 2 for o in released):
                for o in released: o['selected'] = 0
            if not v['success']:
                controls += 1
                self.assertEqual(released, v['after'])
                continue
            constructed += 1
            recoveries += 'Unknown command' in v['history']
            self.assertEqual(v['undo'], released)
            self.assertEqual(v['after'][:-1], released)
            out = v['after'][-1]
            self.assertTrue(out['native_valid'])
            self.assertEqual(out['face_count'], 1)
            self.assertEqual(out['selected'], 0)
            self.assertEqual(out['attributes'], dict(name='', layer='Plane output', color_source='ColorFromLayer', color=[0, 0, 0], groups=[]))
        self.assertEqual((constructed, controls, recoveries), (53, 10, 5))

    def test_standalone_staging_and_existing_plane_primitive_protocol_stay_separate(self):
        query = cases.qualified_request()
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(query, HERE, job)
            self.assertEqual({p.stem for p in job.iterdir()}, set(PROBES['plane_surface_command'][1]) | {'plane_surface_probe'})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys; sys.path.insert(0,sys.argv[1]); import plane_surface_probe', str(job)], check=True, capture_output=True)
        self.assertNotIn('plane_primitive', PROBES)
        validate_operations(json.loads((HERE / 'fixtures/plane_primitives.json').read_text()))

    def test_invalid_recipes_and_nonprivate_or_repeated_requests_fail_before_launch(self):
        seed = cases.qualified_request()
        for change in [dict(case=[]), dict(case=None), dict(id='foreign'), dict(extra='script')]:
            op = copy.deepcopy(seed['operations'][0]); op.update(change)
            with self.assertRaises(ValueError): cases.validate(op)
        with mock.patch.dict('os.environ', {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), mock.patch('subprocess.Popen') as launch:
            with self.assertRaises(OracleProtocolError): OracleClient().run_rhino(seed)
            for iterations in [True, 1.0, 2]:
                q = copy.deepcopy(seed); q['iterations'] = iterations
                with self.assertRaises(OracleProtocolError): OracleClient(settings_scheme='VibocerosOraclePlaneInvalid').run_rhino(q)
            q = copy.deepcopy(seed); q['operations'].append(q['operations'][0])
            with self.assertRaises(OracleProtocolError): OracleClient(settings_scheme='VibocerosOraclePlaneInvalid').run_rhino(q)
            launch.assert_not_called()
