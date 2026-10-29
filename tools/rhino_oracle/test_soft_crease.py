"""Retain soft-crease discovery inputs, native lifecycle and producer bytes."""
from tools.verify_provenance import reconstruct_measurement
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from . import soft_crease_probe as probe
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent
FAMILY = 'soft_crease_discovery'


class SoftCreaseDiscoveryTests(unittest.TestCase):
    def test_chain_qualification_is_complete_historical_and_rejects_bad_certificates(self):
        from .report_subd_chains import summarize
        report=json.loads((HERE.parents[1]/'docs/subd-chains-qualification.json').read_text())
        lines=['SUBD_CHAINS_NATIVE '+json.dumps(row)for row in report['cases']]
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)),report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))),report)
        self.assertEqual((report['workflows'],report['component_sources']),(48,50))
        for invalid in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        for edit in [dict(chains=True),dict(selected_edges=True),dict(topology=False),
                     dict(source_unchanged=1),dict(id='unknown')]:
            row=dict(report['cases'][0],**edit)
            with self.assertRaises(ValueError):
                summarize('\n'.join(['SUBD_CHAINS_NATIVE '+json.dumps(row)]+lines[1:]))

    def test_chain_captures_complete_every_mouse_workflow_and_preserve_producers(self):
        for family,option,count in [('soft_crease_chains','chains',26),
                                    ('soft_crease_followup','followup',18),
                                    ('soft_crease_anchors','anchors',4)]:
            path=HERE/'fixtures'/(family+'.json')
            query=json.loads(path.read_text());self.assertEqual(query,probe.request(**{option:True}))
            native=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(native['results']),count)
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id']for r in native['results']],[o['id']for o in query['operations']])
            for name,digest in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            for row in native['results']:
                value=row['value'];self.assertEqual(value['input'],probe.all_cases()[row['id']])
                trace=value['input_trace'][0]
                self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],trace['expected'])
                for click in trace['clicks']:
                    self.assertTrue(click['motion_acknowledged'] and click['click_acknowledged'])
                    self.assertTrue(trace['prompts'][click['action']+1].startswith('Weight at P'))
                for key,folder in [('source_3dm','source'),('result_3dm','result')]:
                    self.assertEqual((HERE/'fixtures'/family/folder/(row['id']+'.3dm')).read_bytes(),
                                     base64.b64decode(value[key],validate=True))

    def test_moving_label_targets_and_seed_archives_come_from_native_outputs(self):
        assets=json.loads((HERE/'fixtures/soft_crease_followup_assets.json').read_text())
        for data in assets['points'].values():
            ref=data['target_from']
            native=json.loads((HERE/'observations'/(ref['family']+'.json')).read_text())
            value=next(r['value']for r in native['results']if r['id']==ref['id'])
            shape=next(o['subd']for o in value[ref['stage']]if o['source']==ref['source'])
            self.assertEqual(data['point'],shape['surface_points'][ref['vertex']])
        for seed in assets['archives'].values():
            path=HERE/'fixtures'/seed['family']/seed['stage']/(seed['id']+'.3dm')
            data=base64.b64decode(seed['data'],validate=True)
            self.assertEqual(path.read_bytes(),data)
            self.assertEqual(hashlib.sha256(data).hexdigest(),seed['sha256'])

    def native(self):
        return json.loads((HERE / 'observations' / (FAMILY + '.json')).read_text())

    def test_saved_inputs_and_native_archives_are_exact(self):
        path = HERE / 'fixtures' / (FAMILY + '.json')
        query = json.loads(path.read_text())
        self.assertEqual(query, probe.request())
        validate_operations(query)
        native = self.native()
        self.assertEqual(native['engine'], 'rhino')
        self.assertEqual(native['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(native['results']), 32)
        self.assertEqual([r['id'] for r in native['results']],
                         [op['id'] for op in query['operations']])
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),
                         native['provenance']['request_sha256'])
        for row in native['results']:
            self.assertEqual(row['value']['input'], probe.all_cases()[row['id']])
            for key, folder in [('source_3dm', 'source'), ('result_3dm', 'result')]:
                data = (HERE / 'fixtures' / FAMILY / folder / (row['id'] + '.3dm')).read_bytes()
                self.assertEqual(data, base64.b64decode(row['value'][key], validate=True))

    def test_original_producers_remain_available_after_refactoring(self):
        sources = self.native()['provenance']['sources']
        self.assertTrue({'subd_edit_scene.py', 'soft_crease_cases.py',
                         'soft_crease_probe.py', 'rhino_worker.py'} <= set(sources))
        for name, digest in sources.items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest:
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_cancel_and_noop_do_not_consume_a_model_history_record(self):
        no_change = {'soft_constant_0', 'soft_default', 'soft_cancel_empty',
                     'soft_cancel_after_weight', 'soft_boundary',
                     'soft_whole_object', 'soft_vertex_input'}
        cancelled = no_change - {'soft_constant_0', 'soft_default'}
        for row in self.native()['results']:
            key, value = row['id'], row['value']
            events = [e for e in value['events'] if e['name'] == 'SubDCrease']
            self.assertEqual(len(events), 1, key)
            self.assertEqual(events[0]['objects'], value['after'])
            self.assertIs(value['success'], key not in cancelled)
            self.assertEqual(events[0]['result'], 'Cancel' if key in cancelled else 'Success')
            source = lambda stage: next(o for o in value[stage] if o['source'] == 0)
            changed = source('before')['crc'] != source('after')['crc']
            self.assertEqual(changed, key not in no_change, key)
            self.assertEqual(source('after_script')['subd'], source('redo')['subd'])
            self.assertEqual(source('before')['subd'], source('undo')['subd'])
            witnesses = [o for o in value['undo'] if o['name'] == 'Undo witness']
            self.assertEqual(len(witnesses), int(changed), key)

    def test_fixed_validation_and_isolated_helper_imports(self):
        query = probe.request()
        for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different'),
                     dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                probe.validate(bad['operations'][0])
            if 'op' not in edit:
                with self.assertRaises(ValueError):
                    validate_operations(bad)
        modules = ['soft_crease_probe', 'soft_crease_cases', 'soft_crease_chain_cases', 'soft_crease_followup_cases', 'subd_edit_scene',
                   'subdivide_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe',
                   'conic_getter_input', 'managed_mouse_callback', 'snap_environment', 'number_token']
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                             {m + '.py' for m in modules} | {'fixtures'})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules),
                            directory], check=True, capture_output=True)

    def test_every_capture_binds_its_original_recipe_and_producer(self):
        from .report_subd_soft_crease import FAMILIES, DIAGNOSTICS
        cases = probe.all_cases()
        for family, count in FAMILIES + DIAGNOSTICS:
            path = HERE / 'fixtures' / (family + '.json')
            query = json.loads(path.read_text())
            validate_operations(query)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in query['operations']])
            for row in native['results']:
                # The immutable main producer reused one discovery identifier
                # for a seeded source. Current recipes have unique names.
                key = 'seed_soft_default' if family == 'soft_crease' and row['id'] == 'soft_default' else row['id']
                self.assertEqual(row['value']['input'], cases[key])
                for key, folder in [('source_3dm', 'source'), ('result_3dm', 'result')]:
                    data = (HERE / 'fixtures' / family / folder / (row['id'] + '.3dm')).read_bytes()
                    self.assertEqual(data, base64.b64decode(row['value'][key], validate=True))
            for name, digest in native['provenance']['sources'].items():
                source = HERE / 'observations/source_snapshots' / (digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(source.read_bytes())).hexdigest(), digest, name)

    def report_lines(self):
        report = json.loads((HERE.parents[1] / 'docs/subd-soft-crease-qualification.json').read_text())
        lines = ['SOFT_KERNEL ' + json.dumps(row) for row in report['stages']['kernel']]
        lines += ['SOFT_IO ' + json.dumps(row) for row in report['stages']['archive']]
        return report, lines

    def test_geometry_and_archive_reports_are_complete_and_historical(self):
        from .report_subd_soft_crease import summarize
        report, lines = self.report_lines()
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)), report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))), report)
        self.assertEqual(report['compared_workflows'], 133)
        self.assertEqual(report['component_sources'], 138)
        self.assertEqual(len(report['stages']['archive']), 18)
        self.assertFalse(report['command_registered'])
        self.assertFalse(report['full_native_parity'])
        self.assertGreater(report['maximum_native_roundtrip_surface_change'], 1e-6)

    def test_missing_or_invalid_geometry_cannot_qualify(self):
        from .report_subd_soft_crease import summarize
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        first = report['stages']['kernel'][0]
        for change in [dict(sharpness=False), dict(source_unchanged=1), dict(source=True),
                       dict(id='unknown'), dict(changed=1), dict(native_archive_normalizes_sharpness=1)]:
            row = copy.deepcopy(first)
            row.update(change)
            with self.assertRaises(ValueError, msg=change):
                summarize('\n'.join(['SOFT_KERNEL ' + json.dumps(row)] + lines[1:]))
        for field, bad in [('maximum_sampled_surface_error', float('nan')),
                           ('maximum_sampled_surface_error', 1.1e-6),
                           ('maximum_control_or_limit_point_error', -1.),
                           ('vertices', True), ('qualified', False)]:
            row = copy.deepcopy(first)
            row['check'][field] = bad
            with self.assertRaises(ValueError, msg=(field, bad)):
                summarize('\n'.join(['SOFT_KERNEL ' + json.dumps(row)] + lines[1:]))


if __name__ == '__main__':
    unittest.main()
