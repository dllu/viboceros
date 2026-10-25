"""Immutable native grip inputs and independent measured-state comparisons."""
from copy import deepcopy
import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from .extract_points_cases import FAMILIES, request, validate
from .report_extract_points import ROOT, canonical, compare, native, summarize
from .structural_sdk import stage_helpers, validate_operations


class ExtractPointsTests(unittest.TestCase):
    def test_target_layer_diagnostics_retain_rejected_native_option(self):
        here=ROOT/'tools/rhino_oracle'
        for family in ['target_discovery','target_echo','target_menu']:
            name='extract_points_'+family
            data=json.loads((here/'observations'/(name+'.json')).read_text())
            query=here/'fixtures'/(name+'.json')
            self.assertEqual(json.loads(query.read_text()),request(family))
            self.assertEqual(hashlib.sha256(query.read_bytes()).hexdigest(),data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                archived=gzip.decompress((here/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(archived).hexdigest(),digest)
            for row in data['results']:
                self.assertEqual(row['value']['input'],FAMILIES[family][row['id']])
                if family!='target_discovery':
                    self.assertIn('Unknown command: _',row['value']['history'])
                    self.assertIn('TargetObject',row['value']['history'])

    def test_native_scenes_original_producers_and_bounded_requests(self):
        _, rows, _ = native()
        self.assertEqual(len(rows),63)
        for family in FAMILIES:
            query=request(family)
            validate_operations(query)
            for operation in query['operations']:
                with self.assertRaises(ValueError):validate(dict(operation,script='execute'))
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(request(),ROOT/'tools/rhino_oracle',Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {'extract_points_probe.py',
                'extract_points_cases.py','extract_points_display_cases.py','extract_points_subd_cases.py','extract_points_order_cases.py','extract_points_curve_range_input.py','grip_navigation_probe.py','grip_transform_probe.py',
                'join_probe.py','merge_edges_probe.py','extract_points_geometry.py','extract_points_getter.py','managed_component_input.py',
                'extract_points_component_input.py','extract_points_component_cases.py','extract_points_brep_cases.py','extract_points_target_cases.py','extract_points_range_cases.py','extract_points_edge_range_input.py','owned_brep_command.py'})

    def test_measured_geometry_metadata_selection_and_grips_are_compared(self):
        _, rows, _=native()
        for field in ['point','group','selection','hidden','grip_selection']:
            expected=canonical(rows['getter_curve_control']['after'])
            actual=deepcopy(expected)
            if field=='point':actual[-1]['points'][0][0]+=1.
            elif field=='group':actual[-1]['groups']=[]
            elif field=='selection':actual[-1]['selected']=False
            elif field=='hidden':actual[0]['grips'][1]['hidden']=False
            else:actual[0]['grips'][0]['selected']=True
            with self.assertRaises(ValueError,msg=field):compare(actual,expected)

    def test_report_rebuild_and_rejection_of_missing_or_forged_measurements(self):
        record=json.loads((ROOT/'docs/extract-points-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps(r) for r in rows)
        command=log(record['commands'],'EXTRACT_POINTS_GRIPS_COMMAND')
        getter=log(record['getters'],'EXTRACT_POINTS_GRIPS_GETTER')
        self.assertEqual(summarize(command,getter),record)
        for field in ['source_storage_unchanged','success']:
            rows=deepcopy(record['commands']);rows[0][field]=False
            with self.assertRaises(ValueError):summarize(log(rows,'EXTRACT_POINTS_GRIPS_COMMAND'),getter)
        with self.assertRaises(ValueError):summarize(command,log(record['getters'][:-1],'EXTRACT_POINTS_GRIPS_GETTER'))
        with self.assertRaises(ValueError):summarize(command+'\n'+command,getter)
