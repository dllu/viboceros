"""Native extraction state, copied channels and immutable capture provenance."""
import copy
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .extract_analysis_mesh_probe import validate
from .generate_extract_analysis_mesh import FAMILY, request


class ExtractAnalysisMeshTests(unittest.TestCase):
    def test_native_copy_attributes_colors_selection_and_history(self):
        path=HERE/('fixtures/'+FAMILY+'.json')
        q=json.loads(path.read_text())
        n=json.loads(gzip.decompress((HERE/('observations/'+FAMILY+'.json.gz')).read_bytes()))
        self.assertEqual(q,request());self.assertEqual(len(q['operations']),53)
        self.assertEqual(n['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        for name,sha in n['provenance']['sources'].items():
            data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
            if name.endswith('.3dm'):self.assertEqual(data,(HERE/name).read_bytes())
        self.assertEqual(len(n['results']),53)
        for op,row in zip(q['operations'],n['results']):
            with self.subTest(case=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
                states=row['value']['states'];self.assertEqual(len(states),len(op['steps'])+1)
                count=0
                unit=0 if op['source'] in ('line','mesh') else (2 if op['source']=='block_multi' else 1)
                for step,state,trace in zip(op['steps'],states[1:],row['value']['traces']):
                    self.assertTrue(trace['success']);self.assertTrue(all(e['result']=='Success' for e in trace['events']))
                    if step=='Extract':count+=unit
                    elif step=='Undo' and count:count-=unit
                    elif step=='Redo':count+=unit
                    self.assertEqual(len(state['outputs']),count)
                    self.assertEqual(state['source']['selected'],op['preselect'])
                    for out in state['outputs']:
                        attrs,m=out['attributes'],out['mesh']
                        self.assertEqual(attrs['groups'],[]);self.assertFalse(attrs['selected']);self.assertIsNone(attrs['geometry_text'])
                        self.assertEqual(m['normals'],m['vertices'])
                        colored=op['mode'] in ('Draft','Curvature')
                        self.assertEqual(m['colors'],m['vertices'] if colored else 0)
                        if not op['source'].startswith('block'):
                            for key in ('name','layer','wire_density','color','color_source','user_text'):
                                self.assertEqual(attrs[key],state['source'][key])
                    if len(state['cache'])==1 and len(state['outputs'])==1:
                        self.assertEqual(state['cache'][0],state['outputs'][0]['mesh'])

    def test_invalid_recipes_reject_unbounded_or_ambiguous_input(self):
        base=request()['operations'][0]
        for changes in ({'id':'../file'},{'source':'bad'},{'mode':'bad'},{'preselect':1},
                        {'steps':[]},{'steps':['Delete']},{'steps':['Extract']*9},{'extra':True}):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(copy.deepcopy(base),**changes))


if __name__=='__main__':unittest.main()
