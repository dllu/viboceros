"""Fixed torus captures, historical producers and strict qualification gates."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_torus_probe as probe, subd_torus_references_probe as references
from .report_subd_torus import ROOT, HERE, FAMILIES, EXPLORATORY, summarize, validate_input_trace
from .structural_sdk import stage_helpers, validate_operations


class SubDTorusTests(unittest.TestCase):
    def test_fixed_inputs_native_lifecycle_and_archived_sources(self):
        for family, count in FAMILIES + EXPLORATORY:
            request = references.request() if '_reference' in family else probe.request(discovery=family.endswith('discovery'), sizes=family.endswith('sizes'), options=family.endswith('options'))
            path = HERE / 'fixtures' / (family + '.json')
            self.assertEqual(json.loads(path.read_text()), request)
            validate_operations(request)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in request['operations']])
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            if (family, count) not in FAMILIES:
                continue
            for row in native['results']:
                v = row['value']
                recipes = references.SCENES if family.endswith('references') else probe.SCENES
                self.assertEqual(v['input'], recipes[row['id']])
                self.assertEqual('Unknown command:' in v['history'], False)
                self.assertEqual(sum(e['name'] == 'SubDTorus' for e in v['events']), 1)
                self.assertEqual(v['after_script'], v['redo'])
                for phase in ['after_script', 'undo', 'redo']:
                    self.assertEqual({o.get('source', i): o['crc'] for i, o in enumerate(v['before'])}, {o.get('source', i): o['crc'] for i, o in enumerate(v[phase]) if o['role'] == 'source'})

    def test_contact_receipts_and_full_input_trace_are_required(self):
        native = json.loads((HERE / 'observations/subd_torus_references.json').read_text())
        value = next(r['value'] for r in native['results'] if r['id'] == 'torus_tangent_two')
        validate_input_trace(value)
        for edit in ['trace', 'click', 'motion', 'acknowledgement', 'projection', 'completion', 'error']:
            bad = copy.deepcopy(value)
            trace = bad['input_trace'][0]
            if edit == 'trace':
                bad['input_trace'] = []
            elif edit == 'click':
                trace['clicks'].pop()
            elif edit == 'motion':
                trace['clicks'][0]['motion_acknowledged'] = False
            elif edit == 'acknowledgement':
                trace['clicks'][0]['click_acknowledged'] = False
            elif edit == 'projection':
                trace['clicks'][0]['projected_point'] = None
            elif edit == 'completion':
                trace['completed'] -= 1
            else:
                trace['errors'].append('injected failure')
            with self.assertRaises(ValueError, msg=edit):
                validate_input_trace(bad)

    def test_isolated_staging_and_invalid_requests_are_rejected(self):
        for request, modules in [
            (probe.request(), ['subd_torus_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe']),
            (references.request(), ['subd_torus_references_probe', 'subd_cylinder_references_probe', 'subd_sphere_references_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe', 'conic_getter_input', 'managed_mouse_callback', 'snap_environment']),
        ]:
            with tempfile.TemporaryDirectory() as directory:
                job = Path(directory)
                stage_helpers(request, HERE, job)
                self.assertEqual({p.name for p in job.iterdir()}, {m + '.py' for m in modules})
                subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules), str(job)], check=True, capture_output=True, text=True)
            for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different')]:
                bad = copy.deepcopy(request)
                bad['operations'][0].update(edit)
                with self.assertRaises(ValueError):
                    validate_operations(bad)

    def report_lines(self):
        report = json.loads((ROOT / 'docs/subd-torus-qualification.json').read_text())
        return report, ['SUBD_TORUS_' + stage.upper() + ' ' + json.dumps(row) for stage, rows in report['stages'].items() for row in rows]

    def test_complete_report_and_parallel_output_order(self):
        report, lines = self.report_lines()
        self.assertEqual(report, summarize('\n'.join(lines)))
        self.assertEqual(report, summarize('\n'.join(reversed(lines))))
        self.assertEqual((report['command_cases'], report['getter_cases']), (59, 77))
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_missing_failed_or_invalid_checks_cannot_qualify(self):
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for key, bad in [('qualified', False), ('maximum_control_or_limit_point_error', True), ('maximum_sampled_surface_error', float('nan')), ('maximum_sampled_surface_error', 1.1e-6), ('faces', 0), ('surface_sampled', False), ('surface_facets_correspond', False), ('compared_surface_faces', 0)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check']['objects'][0][key] = bad
            data = lines[:]
            data[0] = 'SUBD_TORUS_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))

    def test_diagnostic_cancellations_and_declined_fits_are_explicit(self):
        report, lines = self.report_lines()
        record = report['diagnostic_cancellations']
        self.assertTrue(record['excluded_from_supported_comparisons'])
        self.assertEqual(len(record['actions']), 2)
        self.assertEqual({a['action'] for a in record['actions']}, {'Escape'})
        self.assertEqual(len(report['exploratory_recipes']), 67)
        self.assertEqual(report['successful_constructions'], 58)
        self.assertEqual(report['getter_only_recoveries'], 1)
        self.assertNotIn('subd_torus_default_small_retry', {r['id'] for r in report['stages']['command']})
        for i, line in enumerate(lines):
            if not line.startswith('SUBD_TORUS_REFERENCE_COMMAND '):
                continue
            row = json.loads(line.split(' ', 1)[1])
            if row.get('declined'):
                row['declined'] = False
                bad = lines[:]
                bad[i] = 'SUBD_TORUS_REFERENCE_COMMAND ' + json.dumps(row)
                with self.assertRaises(ValueError):
                    summarize('\n'.join(bad))



if __name__ == '__main__':
    unittest.main()
