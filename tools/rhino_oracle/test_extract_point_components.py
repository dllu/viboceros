"""Reject missing native evidence and forged component measurements."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from .report_extract_point_components import ROOT, FAMILIES, summarize
from .report_extract_points import native, validate_component_getter


class ExtractPointComponentTests(unittest.TestCase):
    def test_native_requests_archives_and_component_states(self):
        _,rows,_=native(FAMILIES,components=True)
        self.assertEqual(len(rows),138)
        from .extract_points_cases import ALL_CASES, FAMILIES as all_families
        self.assertEqual(len(ALL_CASES),sum(len(cases) for cases in all_families.values()))
        for label,row in rows.items():
            if row['input'].get('component_records'):
                self.assertTrue(row['component_records'],label)
            self.assertFalse(any(o['components'] for stage in ['undo','redo'] for o in row[stage]))

    def test_getter_receipt_requires_every_component_and_successful_command(self):
        observation = json.loads((ROOT/'tools/rhino_oracle/observations/extract_points_component_picked_controls.json').read_text())
        row = observation['results'][0]
        validate_component_getter(row['value'], row['id'])
        for key, value in [('component_admitted', [False]), ('command_success', False),
                           ('command_count', 0), ('markers', []), ('getter_counts', []),
                           ('menus', [])]:
            forged = deepcopy(row['value'])
            forged['getter'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_component_getter(forged, row['id'])

    def test_single_face_owner_promotion_is_explicit_and_bounded(self):
        data=json.loads((ROOT/'tools/rhino_oracle/observations/extract_points_component_brep_picked.json').read_text())
        row=next(r for r in data['results'] if r['id']=='postselected_component_brep_surface_edge')
        self.assertEqual(row['value']['getter']['component_admitted'],[False])
        self.assertEqual(row['value']['getter']['owner_promoted'],[True])
        validate_component_getter(row['value'],row['id'])
        for field,value in [('owner_promoted',[False]),('component_admitted',[True]),
                            ('owner_promoted',[]),('owner_promoted',[1])]:
            forged=deepcopy(row['value']);forged['getter'][field]=value
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):
                validate_component_getter(forged,row['id'])
        forged=deepcopy(row['value']);forged['input']['sources'][0]['kind']='brep_box'
        with self.assertRaises(ValueError):validate_component_getter(forged,row['id'])

    def test_report_rebuild_and_rejection_of_missing_or_forged_measurements(self):
        record=json.loads((ROOT/'docs/extract-point-components-qualification.json').read_text())
        def log(rows,prefix):return '\n'.join(prefix+' '+json.dumps({k:v for k,v in r.items() if k!='maximum_deviation'}) for r in rows)
        commands=log(record['commands'],'EXTRACT_POINTS_COMPONENT_COMMAND')
        getters=log(record['getters'],'EXTRACT_POINTS_COMPONENT_GETTER')
        edges=log(record['edge_forms'],'EXTRACT_POINTS_EDGE_FORM')
        self.assertEqual(reconstruct_measurement(record, summarize, commands,getters,edges),record)
        with self.assertRaises(ValueError):summarize(commands,getters,log(record['edge_forms'][:-1],'EXTRACT_POINTS_EDGE_FORM'))
        with self.assertRaises(ValueError):summarize(commands,getters+'\n'+getters,edges)
        forged=deepcopy(record['getters']);forged[0]['after'][0]['components']=[]
        with self.assertRaises(ValueError):summarize(commands,log(forged,'EXTRACT_POINTS_COMPONENT_GETTER'),edges)
        forged=deepcopy(record['edge_forms']);forged[0]['controls'][1][0]+=1.
        with self.assertRaises(ValueError):summarize(commands,getters,log(forged,'EXTRACT_POINTS_EDGE_FORM'))
