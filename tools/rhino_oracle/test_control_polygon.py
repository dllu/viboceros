"""Fixed input, original producer and native command admission checks."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from . import control_polygon_cases as cases
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


class ControlPolygonTests(unittest.TestCase):
    def test_whole_object_report_requires_all_command_and_ui_cases(self):
        from .report_control_polygon import ROOT, summarize
        report = json.loads((ROOT/'docs/control-polygon-qualification.json').read_text())
        command = ['CONTROL_POLYGON_NATIVE '+json.dumps(r) for r in report['comparisons']['command']]
        ui = ['CONTROL_POLYGON_UI '+json.dumps(r) for r in report['comparisons']['ui']]
        self.assertEqual(summarize('\n'.join(command), '\n'.join(ui)), report)
        self.assertEqual((report['compared_workflows'], report['successful_workflows'],
                          report['rejected_workflows'], len(report['diagnostic_workflows'])),
                         (42, 32, 10, 19))
        self.assertTrue(report['whole_object_command_qualified'])
        self.assertTrue(report['interactive_object_getter_qualified'])
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for a, b in [(command[:-1], ui), (command, ui[:-1]), (command+command[:1], ui),
                     (command, ui+ui[:1])]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(a), '\n'.join(b))
        for field, value in [('maximum_error', float('nan')), ('maximum_error', 2e-6),
                             ('geometry_unchanged', False), ('metadata_exact', False),
                             ('selection_exact', False), ('history_exact', False),
                             ('qualified', True)]:
            rows = copy.deepcopy(report['comparisons']['command'])
            rows[0][field] = value
            if field == 'qualified':
                rows[0]['success'] = not rows[0]['success']
            bad = '\n'.join('CONTROL_POLYGON_NATIVE '+json.dumps(r) for r in rows)
            with self.assertRaises(ValueError):
                summarize(bad, '\n'.join(ui))

    def test_control_net_report_requires_complete_exact_attributes(self):
        from .report_subd_control_net import ROOT,summarize
        report = json.loads((ROOT/'docs/subd-control-net-qualification.json').read_text())
        lines = ['CONTROL_NET_NATIVE '+json.dumps(row) for row in report['cases']]
        self.assertEqual(summarize('\n'.join(lines)),report)
        self.assertEqual((report['compared_workflows'],report['component_sources']),(20,21))
        for bad in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad))
        for field,value in [('qualified',False),('source_unchanged',1),('vertices',True),
                            ('topology_exact',False),('vertex_attributes_exact',False),
                            ('maximum_point_error',float('nan')),('maximum_point_error',1.01e-6)]:
            row = copy.deepcopy(report['cases'][0])
            row[field] = value
            with self.assertRaises(ValueError,msg=field):
                summarize('\n'.join(['CONTROL_NET_NATIVE '+json.dumps(row)]+lines[1:]))

    def test_owned_capture_rejects_unbounded_or_mixed_requests_before_launch(self):
        from .client import OracleClient, OracleProtocolError
        query = cases.request()
        invalid = []
        for iterations in [True,2]:
            invalid.append(dict(copy.deepcopy(query),iterations=iterations))
        duplicate = copy.deepcopy(query)
        duplicate['operations'].append(duplicate['operations'][0])
        invalid.append(duplicate)
        mixed = copy.deepcopy(query)
        mixed['operations'].append(dict(op='line_length',id='other'))
        invalid.append(mixed)
        with patch.dict('os.environ',{'DISPLAY':':917','VIBOCEROS_ORACLE_HEADLESS':':917'}), \
                patch('subprocess.Popen') as launch:
            for request in invalid:
                with self.assertRaises((ValueError,OracleProtocolError)):
                    OracleClient(settings_scheme='VibocerosOracleControlPolygonTest').run_rhino(request)
            with self.assertRaises(OracleProtocolError):
                OracleClient().run_rhino(query)
            launch.assert_not_called()

    def test_complete_fixed_requests_and_frozen_native_producers(self):
        for suffix, request, count in [('', cases.request, 37), ('_options', cases.options_request, 12),
                                        ('_priority', cases.priority_request, 12),
                                        ('_components', cases.components_request, 14), ('_picks', cases.picks_request, 2)]:
            family = 'control_polygon'+suffix
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_text())
            self.assertEqual(query,request())
            validate_operations(query)
            native = json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(native['results']),count)
            self.assertEqual([r['id'] for r in native['results']],[o['id'] for o in query['operations']])
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in native['provenance']['sources'].items():
                original = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(original).hexdigest(),digest,name)
            for row in native['results']:
                self.assertNotIn('error',row)
                value = row['value']
                self.assertEqual(value['input'],cases.SCENES[row['id']])
                events = [e for e in value['events'] if e['name']=='ExtractControlPolygon']
                self.assertEqual(len(events),1,row['id'])
                before = {o['source']:o['crc'] for o in value['before'] if o['source'] is not None}
                after = {o['source']:o['crc'] for o in value['after'] if o['source'] is not None}
                self.assertEqual(before,after,row['id'])
                for stage in ['source','result']:
                    self.assertTrue((HERE/'fixtures'/family/stage/(row['id']+'.3dm')).is_file())

    def test_arbitrary_operations_and_unstaged_imports_are_rejected(self):
        query = cases.request()
        for change in [dict(case='eval'),dict(case=[]),dict(id='different'),dict(extra=True)]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(change)
            with self.assertRaises(ValueError):
                validate_operations(bad)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                {'control_polygon_probe.py','control_polygon_cases.py','join_probe.py','merge_edges_probe.py',
                 'owned_brep_command.py','shrink_face_input.py'})

    def test_range_report_requires_both_getter_paths_and_all_component_cases(self):
        from .report_control_polygon_ranges import ROOT, summarize
        report = json.loads((ROOT/'docs/control-polygon-ranges-qualification.json').read_text())
        command = ['CONTROL_POLYGON_RANGE_NATIVE '+json.dumps(r) for r in report['comparisons']['command']]
        ui = ['CONTROL_POLYGON_RANGE_UI '+json.dumps(r) for r in report['comparisons']['ui']]
        ui += ['CONTROL_POLYGON_COMPONENT_UI '+json.dumps(r) for r in report['comparisons']['components']]
        self.assertEqual(summarize('\n'.join(command),'\n'.join(ui)),report)
        self.assertEqual((report['temporary_ranges'],report['interactive_range_paths'],report['component_workflows']),(5,10,10))
        for a,b in [(command[:-1],ui),(command,ui[:-1]),(command+command[:1],ui),(command,ui+ui[:1])]:
            with self.assertRaises(ValueError): summarize('\n'.join(a),'\n'.join(b))
        for field,value in [('maximum_error',float('nan')),('maximum_error',2e-6),('metadata_exact',False),('history_exact',1)]:
            rows=copy.deepcopy(report['comparisons']['command']);rows[0][field]=value
            with self.assertRaises(ValueError):
                summarize('\n'.join('CONTROL_POLYGON_RANGE_NATIVE '+json.dumps(r) for r in rows),'\n'.join(ui))
