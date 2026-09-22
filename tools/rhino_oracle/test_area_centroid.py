"""Source fidelity, replay scope and independently derived area moments."""
import copy
import hashlib
import json
import unittest
from pathlib import Path
from .area_centroid_probe import validate
from .area_centroid_replay import prepare
from .client import OracleProtocolError
from .references.area_centroid import request
from .references.area_centroid_integrals import reference

ROOT=Path(__file__).resolve().parents[2]


def inputs():
    return [json.loads((ROOT/('tools/rhino_oracle/'+directory+'/area_centroid.json')).read_text()) for directory in ('fixtures','observations')]


class AreaCentroidTests(unittest.TestCase):
    def test_all_sources_regenerate_without_observation_targets(self):
        data,observed=inputs()
        self.assertEqual(data,request())
        self.assertEqual(len(data['operations']),54)
        native,evidence=prepare(data,observed)
        self.assertEqual(native,data)
        self.assertEqual(len(evidence['results']),54)
        for row in observed['results']:
            for point in row['value']['points']: point['point']=[123.,456.,789.]
            for mass in row['value']['properties']:
                if mass is not None: mass['centroid']=[123.,456.,789.]
        self.assertEqual(prepare(data,observed)[0],native)

    def test_replay_keeps_default_command_and_explicit_tolerance_api_separate(self):
        data,observed=inputs()
        _,default=prepare(data,observed)
        _,tight=prepare(data,observed,True)
        for source,a,b in zip(observed['results'],default['results'],tight['results']):
            self.assertEqual(set(a['value']),{'properties','points','selected','succeeded'})
            self.assertEqual(b['value'],dict(properties=source['value']['tight_properties']))
        rows={r['id']:r['value'] for r in observed['results']}
        self.assertEqual(rows['mixed-open']['selected'],[0,1])
        self.assertFalse(rows['open-only']['succeeded'])
        self.assertEqual(rows['post-separate']['selected'],[])
        self.assertEqual([r['id'] for r in observed['results'] if 'reversed' in r['value']['insertions']],['brep-paraboloid-capped-inward'])

    def test_mesh_inputs_are_retained_at_binary64_precision(self):
        data,observed=inputs()
        checked=0
        for op,row in zip(data['operations'],observed['results']):
            for source,stored in zip(op['sources'],row['value']['inputs']):
                if source['type']=='mesh':
                    self.assertEqual(stored,{k:source[k] for k in ('vertices','faces')})
                    checked+=1
        self.assertEqual(checked,35)

    def test_invalid_fields_ids_and_indices_fail_before_owned_input(self):
        data,_=inputs(); good=data['operations'][0]
        for change in [dict(op='other'),dict(id='bad command'),dict(preselect=1),dict(selected=[]),
                       dict(selected=[True]),dict(selected=[99]),dict(selected=[0,0]),dict(selected=[[]]),dict(groups=[[0,99]]),dict(extra=True)]:
            with self.assertRaises(ValueError): validate(dict(good,**change))

    def test_tampered_geometry_and_order_are_not_replayed(self):
        data,observed=inputs()
        wrong=copy.deepcopy(observed); wrong['results'].reverse()
        with self.assertRaises(OracleProtocolError): prepare(data,wrong)
        mesh=next(i for i,o in enumerate(data['operations']) if o['sources'][0]['type']=='mesh')
        for field in ('inputs','constructed_inputs'):
            wrong=copy.deepcopy(observed)
            wrong['results'][mesh]['value'][field][0]['vertices'][0][0]=1.
            with self.assertRaises(OracleProtocolError): prepare(data,wrong)
        for iteration in (True,0,2):
            with self.assertRaises(OracleProtocolError): prepare(dict(data,iterations=iteration),observed)
        reversed_index=next(i for i,r in enumerate(observed['results']) if 'reversed' in r['value']['insertions'])
        wrong=copy.deepcopy(observed)
        faces=wrong['results'][reversed_index]['value']['inputs'][0]['topology']['faces']
        faces[0]['reversed']=not faces[0]['reversed']
        with self.assertRaises(OracleProtocolError): prepare(data,wrong)
        for change in (dict(selected=[True]),dict(points=[dict(point=[0.,0.,0.])]),
                       dict(points=None),dict(selected=[0,0])):
            wrong=copy.deepcopy(observed)
            wrong['results'][0]['value'].update(change)
            with self.assertRaises(OracleProtocolError): prepare(data,wrong)

    def test_decimal_reference_regenerates_and_retains_real_default_rhino_residuals(self):
        expected=json.loads((ROOT/'tools/rhino_oracle/fixtures/area_centroid_integrals.json').read_text())
        self.assertEqual(expected,reference())
        _,observed=inputs()
        rows={r['id']:r['value'] for r in observed['results']}
        for name,values in expected['values'].items():
            # Evidence of actual retained default-API errors, not replacement
            # targets or a reason to force native integration toward Rhino.
            actual=rows[name]['properties'][0]
            self.assertGreater(abs(float(values['area'])-actual['area']),1e-9,name)

    def test_provenance_hashes(self):
        provenance=json.loads((ROOT/'docs/area-centroid-provenance.json').read_text())
        for path,digest in provenance['retained_file_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)


if __name__=='__main__': unittest.main()
