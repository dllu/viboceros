"""Fixed reference captures, source staging and cylinder qualification guards."""
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from . import subd_cylinder_references_probe as probe
from .client import OracleClient, OracleProtocolError
from .report_subd_cylinder_references import FAMILIES, ROOT, HERE, summarize
from .structural_sdk import stage_helpers, validate_operations


class SubDCylinderReferenceTests(unittest.TestCase):
    def test_fixed_recipes_archived_producers_and_native_lifecycle(self):
        for index, (family, count) in enumerate(FAMILIES):
            path = HERE / 'fixtures' / (family + '.json')
            request = json.loads(path.read_text())
            self.assertEqual(request, probe.request(extras=bool(index)))
            validate_operations(request)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            for row in native['results']:
                value = row['value']
                self.assertEqual(value['input'], probe.ALL_CASES[row['id']])
                self.assertEqual(sum(e['name'] == 'SubDCylinder' for e in value['events']), 1)
                self.assertNotIn('Unknown command:', value['history'])
                self.assertEqual(value['after_script'], value['redo'])
                for phase in ['after_script', 'undo', 'redo']:
                    self.assertEqual({o['source']: o['crc'] for o in value['before']},
                                     {o['source']: o['crc'] for o in value[phase] if o['role'] == 'source'})

    def test_standalone_helpers_and_changed_requests_are_checked_before_launch(self):
        request = probe.request()
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(request, HERE, job)
            modules = ['subd_cylinder_references_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe',
                       'conic_getter_input', 'managed_mouse_callback', 'snap_environment']
            self.assertEqual({p.name for p in job.iterdir()}, {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules), str(job)],
                           check=True, capture_output=True, text=True)
        for edit in [dict(case='arbitrary'), dict(id='different'), dict(extra=True)]:
            bad = copy.deepcopy(request)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                validate_operations(bad)
        with mock.patch.dict(os.environ, DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), \
             mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            mixed = copy.deepcopy(request)
            mixed['operations'].append(dict(op='subd_cylinder_command', id='subd_cylinder_default', case='subd_cylinder_default'))
            for bad in [dict(request, iterations=2), dict(request, iterations=True), mixed]:
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'), settings_scheme='VibocerosOracleCylinderReferenceUnit').run_rhino(bad, 1)
            launch.assert_not_called()

    def report_lines(self):
        report = json.loads((ROOT / 'docs/subd-cylinder-references-qualification.json').read_text())
        lines = ['SUBD_CYLINDER_REFERENCE_' + stage.upper() + ' ' + json.dumps(row)
                 for stage, rows in report['stages'].items() for row in rows]
        return report, lines

    def test_complete_current_report_and_parallel_result_order(self):
        report, lines = self.report_lines()
        self.assertEqual(report, summarize('\n'.join(lines)))
        self.assertEqual(report, summarize('\n'.join(reversed(lines))))
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_missing_failed_and_invalid_checks_cannot_qualify(self):
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for key, bad in [('qualified', False), ('maximum_control_or_limit_point_error', True),
                         ('maximum_sampled_surface_error', float('nan')), ('maximum_sampled_surface_error', 1.1e-6),
                         ('faces', 0), ('surface_sampled', False)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check']['objects'][0][key] = bad
            data = lines[:]
            data[0] = 'SUBD_CYLINDER_REFERENCE_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        row = copy.deepcopy(report['stages']['command'][-1])
        row['declined'] = False
        with self.assertRaises(ValueError):
            summarize('\n'.join(lines[:len(report['stages']['command'])-1] + ['SUBD_CYLINDER_REFERENCE_COMMAND ' + json.dumps(row)] + lines[len(report['stages']['command']):]))


if __name__ == '__main__':
    unittest.main()
