"""Fixed sphere captures, immutable provenance, staging and qualification guards."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_sphere_probe as probe, subd_sphere_references_probe as references
from .report_subd_sphere import ROOT, HERE, FAMILIES, DIAGNOSTICS, summarize
from .structural_sdk import stage_helpers, validate_operations


class SubDSphereTests(unittest.TestCase):
    def test_fixed_inputs_native_lifecycle_and_archived_producers(self):
        for family, count in FAMILIES + DIAGNOSTICS:
            request = references.request() if family.endswith('references') else probe.request(discovery=family.endswith('discovery'), styles=family.endswith('styles'))
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
                self.assertNotIn('Unknown command:', v['history'])
                self.assertEqual(sum(e['name'] == 'SubDSphere' for e in v['events']), 1)
                self.assertEqual(v['after_script'], v['redo'])
                for phase in ['after_script', 'undo', 'redo']:
                    self.assertEqual({o.get('source', i): o['crc'] for i, o in enumerate(v['before'])},
                                     {o.get('source', i): o['crc'] for i, o in enumerate(v[phase]) if o['role'] == 'source'})

    def test_isolated_staging_and_changed_requests_are_rejected(self):
        for request, modules in [
            (probe.request(), ['subd_sphere_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe']),
            (references.request(), ['subd_sphere_references_probe', 'subd_cylinder_references_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe', 'conic_getter_input', 'managed_mouse_callback', 'snap_environment']),
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
        report = json.loads((ROOT / 'docs/subd-sphere-qualification.json').read_text())
        return report, ['SUBD_SPHERE_' + stage.upper() + ' ' + json.dumps(row)
                        for stage, rows in report['stages'].items() for row in rows]

    def test_historical_complete_report_and_parallel_order(self):
        report, lines = self.report_lines()
        self.assertEqual(report, reconstruct_measurement(report, summarize, '\n'.join(lines)))
        self.assertEqual(report, reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))))
        self.assertEqual((report['command_cases'], report['getter_cases']), (49, 54))
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertGreater(verify_measurement(report), 0)

    def test_missing_failed_or_invalid_checks_cannot_qualify(self):
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for key, bad in [('qualified', False), ('maximum_control_or_limit_point_error', True), ('maximum_sampled_surface_error', float('nan')), ('maximum_sampled_surface_error', 1.1e-6), ('faces', 0), ('surface_sampled', False)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check']['objects'][0][key] = bad
            data = lines[:]
            data[0] = 'SUBD_SPHERE_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        row = copy.deepcopy(report['stages']['reference_command'][0])
        row['grouped_history'] = False
        offset = len(report['stages']['command']) + len(report['stages']['getter'])
        data = lines[:]
        data[offset] = 'SUBD_SPHERE_REFERENCE_COMMAND ' + json.dumps(row)
        with self.assertRaises(ValueError):
            summarize('\n'.join(data))


if __name__ == '__main__':
    unittest.main()
