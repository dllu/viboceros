"""Immutable independent input, calibration-only outputs and strict replay reports."""
from tools.verify_provenance import reconstruct_measurement
import base64
from copy import deepcopy
import gzip
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .insert_point_cases import ALL_CASES, request
from .insert_point_mesh_sdk_cases import request as sdk_request, validate as sdk_validate
from .report_insert_point_command import (
    CELL_GAPS, DEPTH_CELL_GAPS, SPATIAL_CELL_GAPS, SURFACE_GAPS, FAMILIES, REPLAYS, HERE, ROOT, capture, sdk_calibration, summarize,
)
from .report_insert_point_calibration import methods_calibration
from .report_insert_point_inputs import validate_depth
from .structural_sdk import stage_helpers, validate_operations
from .qualification_sources import with_shared_sources

VARIANTS = dict(insert_point_holdouts_v2='holdouts', insert_point_ngons='ngons',
                insert_point_mesh_shapes='shapes', insert_point_pick_depth='depth',
                insert_point_pick_depth_v2='witnesses', insert_point_spatial_layouts='spatial', insert_point_spatial_prefixes='sequences', insert_point_mesh_sdk=None,
                insert_point_mesh_sdk_origins='origins', insert_point_mesh_sdk_methods='methods', insert_point_mesh_sdk_spatial='spatial')


def query_for(family):
    variant = VARIANTS[family]
    create = sdk_request if 'sdk' in family else request
    return create(**({variant: True} if variant else {}))


class InsertPointCommandTests(unittest.TestCase):
    def test_generator_rejects_immutable_recaptures_before_launch_or_fixture_mutation(self):
        from .generate_insert_point import FAMILIES as variants
        invocations = [(family, 'generate_insert_point', [] if flag == 'discovery' else ['--'+flag])
                       for flag, (family, _) in variants.items()]
        invocations += [('insert_point_mesh_sdk'+('_'+flag if flag else ''),
                         'generate_insert_point_mesh_sdk', ['--'+flag] if flag else [])
                        for flag in ['', 'origins', 'methods', 'spatial']]
        for family, module, flags in invocations:
            paths = [HERE/folder/(family+'.json') for folder in ['fixtures', 'observations']]
            before = [p.read_bytes() for p in paths]
            args = [sys.executable, '-m', 'tools.rhino_oracle.'+module]+flags
            result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('observations are immutable', result.stderr)
            self.assertNotIn('Capturing ', result.stdout)
            self.assertEqual([p.read_bytes() for p in paths], before)

    def test_shared_source_expansion_binds_extracted_code_in_one_pass(self):
        for root, required in [
            ('src/app.rs', ['src/app/insert_point.rs', 'src/viewport/insert_point.rs']),
            ('crates/viboceros-geometry/src/loft.rs', [
                'crates/viboceros-geometry/src/loft/parameterize.rs',
                'crates/viboceros-geometry/src/loft/parameterize/tests.rs',
                'crates/viboceros-geometry/src/integration/cash_karp.rs',
                'crates/viboceros-opennurbs/native/curve_speeds.h',
                'crates/viboceros-opennurbs/src/curve/speeds.rs']),
            ('src/viewport.rs', ['src/viewport/insert_point.rs']),
            ('crates/viboceros-geometry/src/mesh.rs', [
                'crates/viboceros-geometry/src/mesh/aspect_ratio.rs',
                'crates/viboceros-geometry/src/mesh/insert/ngons.rs',
                'crates/viboceros-geometry/src/mesh/insert/regions.rs',
                'crates/viboceros-geometry/src/mesh/insert/pairing.rs',
                'crates/viboceros-geometry/src/edge_insertion/polygons.rs']),
            ('crates/viboceros-command/tests/support/insert_point.rs', ['crates/viboceros-command/tests/support/insert_point/mesh.rs']),
            ('crates/viboceros-command/src/lib.rs', ['crates/viboceros-command/src/insert_point/arguments.rs']),
            ('tools/rhino_oracle/insert_point_cases.py', [
                'tools/rhino_oracle/insert_point_ngon_cases.py',
                'tools/rhino_oracle/insert_point_mesh_shape_cases.py',
                'tools/rhino_oracle/insert_point_pick_depth_cases.py']),
        ]:
            paths = with_shared_sources([root])
            self.assertTrue(set(required) <= set(paths))
            self.assertEqual(set(paths), set(with_shared_sources(paths)))

    def test_new_captures_bind_requests_frozen_producers_and_source_archives(self):
        for family in VARIANTS:
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_text())
            self.assertEqual(query, query_for(family))
            validate_operations(query)
            native = capture(family)
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in native['provenance']['sources'].items():
                current = (HERE/name).read_bytes()
                frozen = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), digest)
                if hashlib.sha256(current).hexdigest() == digest:
                    self.assertEqual(current, frozen)
            for row in native['results']:
                self.assertNotIn('error', row)
                if 'sdk' not in family:
                    recipe = deepcopy(row['value']['input'])
                    self.assertEqual(recipe.pop('capture_id'), row['id'])
                    self.assertEqual(recipe, ALL_CASES[row['id']])
                    path = HERE/'fixtures'/family/(row['id']+'.3dm')
                    self.assertEqual(path.read_bytes(), base64.b64decode(row['value']['source_3dm'], validate=True))

    def test_reversed_native_paths_preserve_cells_after_coordinate_correspondence(self):
        from .report_insert_point_calibration import cycle
        spatial = {r['id']: r['value'] for r in capture('insert_point_spatial_layouts')['results']}
        prefixes = {r['id']: r['value'] for r in capture('insert_point_spatial_prefixes')['results']}
        for original, reverse in [
            ('bottom_mid_left_high', 'left_high_bottom_mid'),
            ('same_edge_warp', 'same_edge_reverse'),
        ]:
            def mesh(value):
                return next(o['mesh'] for o in value['after'] if o['source'] == 0)
            a = mesh(spatial['insert_point_spatial_' + original])
            b = mesh(prefixes['insert_point_spatial_prefix_' + reverse])
            remaining = set(range(len(b['vertices'])))
            correspondence = []
            for point in a['vertices']:
                matches = [i for i in remaining if math.dist(point, b['vertices'][i]) <= 1e-6]
                self.assertEqual(len(matches), 1)
                correspondence.append(matches[0]); remaining.remove(matches[0])
            self.assertFalse(remaining)
            self.assertEqual(sorted(cycle([correspondence[i] for i in f]) for f in a['faces']),
                             sorted(cycle(f) for f in b['faces']))

    def test_failed_import_is_a_diagnostic_with_no_surviving_response(self):
        failure = json.loads((HERE/'observations/insert_point_holdouts_import_failure.json').read_text())
        self.assertEqual(failure['status'], 'worker_import_failure')
        self.assertEqual(failure['qualified_workflows'], 0)
        self.assertIs(failure['response_retained'], False)
        for field, path in [('request_sha256', 'fixtures/insert_point_holdouts.json'),
                            ('log_sha256', 'observations/insert_point_holdouts_import_failure.log')]:
            self.assertEqual(failure[field], hashlib.sha256((HERE/path).read_bytes()).hexdigest())
        self.assertFalse((HERE/'observations/insert_point_holdouts.json').exists())
        for digest in failure['sources'].values():
            data = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest)

    def test_standalone_new_probes_and_recipe_allowlists(self):
        for family in VARIANTS:
            query = query_for(family)
            module = 'insert_point_mesh_sdk_probe' if 'sdk' in family else 'insert_point_probe'
            for extra in [{'case': 'arbitrary'}, {'id': 'arbitrary'}, {'path': 'arbitrary'}]:
                op = dict(query['operations'][0], **extra)
                with self.assertRaises(ValueError):
                    validate_operations(dict(query, operations=[op]))
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(query, HERE, Path(directory))
                subprocess.run([sys.executable, '-I', '-c',
                                'import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',
                                directory, module], check=True, capture_output=True)
        for invalid in [None, [], '', 0, True, dict(case=[], methods=True)]:
            with self.assertRaises(ValueError):
                sdk_validate(invalid)
        with self.assertRaises(ValueError):
            sdk_request(origins=True, methods=True)

    def evidence(self):
        report = json.loads((ROOT/'docs/insert-point-command-qualification.json').read_text())
        lines = ['INSERT_POINT_'+stage+' '+json.dumps(r)
                 for stage, key, _ in REPLAYS for r in report[key]]
        return report, lines

    def test_report_reconstructs_all_stages_with_explicit_native_gaps(self):
        report, lines = self.evidence()
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))), report)
        self.assertTrue(report['command_implemented'])
        self.assertEqual((report['command_cases'], report['getter_cases']), (84, 84))
        self.assertEqual(report['additional_geometry_cases'], 12)
        self.assertEqual(report['primary_cell_gaps'], sorted(CELL_GAPS))
        self.assertEqual(report['depth_cell_gaps'], [])
        self.assertEqual(report['spatial_cell_gaps'], sorted(SPATIAL_CELL_GAPS))
        self.assertEqual(report['geometry_unqualified'], sorted(SURFACE_GAPS))
        self.assertGreater(report['maximum_fixed_facet_sample_error'], .1)
        self.assertEqual(report['maximum_error'], report['maximum_fixed_facet_sample_error'])
        self.assertLessEqual(report['maximum_qualified_error'], 1e-6)
        for key in ['holdout_region_gaps', 'holdout_cell_gaps', 'ngon_cell_gaps', 'shape_cell_gaps']:
            self.assertEqual(report[key], [])
        self.assertEqual((report['ngon_cases'], report['ngon_getter_cases']), (12, 12))
        self.assertEqual(report['spatial_cases'], 12)
        self.assertEqual(report['prefix_cases'], 7)
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertEqual(sum(FAMILIES.values()), 180)
        self.assertEqual(report['captured_command_workflows'], 120)
        self.assertEqual(len(lines), 180)

    def test_calibration_does_not_substitute_for_command_input_evidence(self):
        rows = sdk_calibration()
        self.assertEqual(len(rows), 16)
        self.assertEqual(sum(r['api_matches']['polyline_triangles_quads_0.0'] for r in rows), 7)
        self.assertEqual(sum(r['api_matches']['closed_polyline_quads_0.0'] for r in rows), 1)
        self.assertEqual(sum(bool(r['matching_origins']) for r in rows), 7)
        methods = methods_calibration(capture)
        self.assertEqual(len(methods), 16)
        self.assertEqual(sum(r['api_matches']['tessellation_free_quads'] for r in methods), 1)
        self.assertEqual(sum(r['api_matches']['tessellation_boundary_quads'] for r in methods), 1)
        self.assertTrue(all(r['vertex_counts']['lines_4_mesh'] is None for r in methods))
        for key in methods[0]['api_matches']:
            self.assertLess(sum(r['api_matches'][key] for r in methods), 16)
        from .report_insert_point_calibration import spatial_calibration
        spatial=spatial_calibration(capture)
        self.assertEqual(len(spatial),12)
        for r in spatial:
            if 'left_high' in r['id'] or 'same_edge_warp' in r['id']:self.assertEqual(r['matching_cells'],[])
        report, _ = self.evidence()
        self.assertEqual(report['spatial_sdk_calibration'],spatial)
        self.assertEqual(report['sdk_calibration'], rows)
        self.assertEqual(report['methods_calibration'], methods)
        self.assertEqual(report['calibration_recipes'], 60)

    def test_every_stage_requires_overlay_storage_and_finite_geometry_evidence(self):
        report, lines = self.evidence()
        for stage, field, _ in REPLAYS:
            row = report[field][0]
            needle = 'INSERT_POINT_'+stage+' '+json.dumps(row)
            remaining = [line for line in lines if line != needle]
            for text in ['\n'.join(remaining), '\n'.join(lines+[needle])]:
                with self.assertRaises(ValueError):
                    summarize(text)
            for key, value in [('ngon_overlays_correspond', False), ('ngons', False),
                               ('qualified', False), ('maximum_error', float('nan')),
                               ('stored_pieces_correspond', not row['comparison']['stored_pieces_correspond']),
                               ('stored_facets_correspond', not row['comparison']['stored_facets_correspond'])]:
                # Holdout SubD has its own sampled-surface schema; first rows here are meshes.
                bad = deepcopy(row)
                bad['comparison'][key] = value
                with self.assertRaises(ValueError):
                    summarize('\n'.join(remaining+['INSERT_POINT_'+stage+' '+json.dumps(bad)]))

    def test_missing_nonfinite_and_relabelled_replays_fail(self):
        report, lines = self.evidence()
        row = next(r for r in report['cases'] if r['id'] in CELL_GAPS)
        needle = 'INSERT_POINT_COMMAND '+json.dumps(row)
        remaining = [line for line in lines if line != needle]
        for field, value in [('history', False), ('preview', False), ('identity', True+0),
                             ('changed', not row['changed']), ('id', 'unrelated')]:
            bad = deepcopy(row)
            bad[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join(remaining+['INSERT_POINT_COMMAND '+json.dumps(bad)]))
        for field, value in [('maximum_error', float('nan')), ('maximum_error', float('inf')),
                             ('maximum_error', 1e-3), ('vertices', True), ('faces', -1),
                             ('channels', False), ('logical_regions', False),
                             ('ngon_overlays_correspond', False), ('ngons', True),
                             ('stored_pieces_correspond', True), ('stored_facets_correspond', True),
                             ('native_cells', []), ('actual_cells', [])]:
            bad = deepcopy(row)
            bad['comparison'][field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join(remaining+['INSERT_POINT_COMMAND '+json.dumps(bad)]))

    def test_depth_witnesses_reject_missing_nonfinite_and_wrong_target_planes(self):
        click = capture('insert_point_pick_depth_v2')['results'][0]['value']['input_trace'][0]['clicks'][0]
        validate_depth(click, required=True, diagnostic=True)
        for field in ['view_target', 'camera_direction', 'view_target_point', 'ray_hit_point']:
            missing = dict(click)
            del missing[field]
            with self.assertRaises(ValueError):
                validate_depth(missing, required=True, diagnostic=True)
            for invalid in [[0., 0.], [0., 0., float('nan')], [True, 0., 1.]]:
                with self.assertRaises(ValueError):
                    validate_depth(dict(click, **{field: invalid}), required=True, diagnostic=True)
        with self.assertRaises(ValueError):
            validate_depth(dict(click, view_target_point=[0., 0., 0.]), required=True, diagnostic=True)
        # The public SDK hit must never replace the native display-facet hint.
        validate_depth(dict(click, ray_hit_point=[1., 2., 3.]), required=True, diagnostic=True)

    def test_warped_display_surface_gaps_cannot_be_relabelled_as_qualified(self):
        report, lines = self.evidence()
        for stage, field, _ in REPLAYS:
            for row in report[field]:
                if row['id'] not in SURFACE_GAPS:
                    continue
                self.assertIs(row['comparison']['qualified'], False)
                self.assertIs(row['comparison']['fixed_facet_surface_qualified'], False)
                self.assertGreater(row['comparison']['maximum_fixed_facet_sample_error'], 1e-6)
                needle = 'INSERT_POINT_'+stage+' '+json.dumps(row)
                remaining = [line for line in lines if line != needle]
                for key, value in [('qualified', True), ('fixed_facet_surface_qualified', True),
                                   ('maximum_fixed_facet_sample_error', 0.), ('maximum_error', 0.),
                                   ('maximum_fixed_facet_sample_error', float('nan'))]:
                    bad = deepcopy(row)
                    bad['comparison'][key] = value
                    with self.assertRaises(ValueError):
                        summarize('\n'.join(remaining+['INSERT_POINT_'+stage+' '+json.dumps(bad)]))


if __name__ == '__main__':
    unittest.main()
