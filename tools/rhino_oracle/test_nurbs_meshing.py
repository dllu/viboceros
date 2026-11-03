"""Frozen public native density controls, geometric witnesses and provenance."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_nurbs_meshing import request
from .nurbs_meshing_probe import SOURCES, PROFILES, validate


class NurbsMeshingTests(unittest.TestCase):
    def test_native_capture_has_all_fixed_cases_and_frozen_producers(self):
        fixture=HERE/'fixtures/nurbs_meshing.json'
        query=json.loads(fixture.read_text())
        native=json.loads((HERE/'observations/nurbs_meshing.json').read_text())
        self.assertEqual(query,request())
        self.assertEqual(len(query['operations']),54)
        self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
        self.assertEqual([r['id'] for r in native['results']],[o['id'] for o in query['operations']])
        for name,digest in native['provenance']['sources'].items():
            data=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
        for row in native['results']:
            self.assertNotIn('error',row)
            self.assertTrue(row['value']['meshes'])

    def test_public_density_parameters_and_native_geometry_dependent_counts(self):
        native=json.loads((HERE/'observations/nurbs_meshing.json').read_text())
        rows={r['id']:r['value'] for r in native['results']}
        for source in SOURCES:
            counts=[]
            for profile in PROFILES[:5]:
                row=rows[source+'-'+profile]
                density=int(profile[7:])/100.
                self.assertEqual(row['parameters']['RelativeTolerance'],density)
                self.assertEqual(row['parameters']['SimplePlanes'],density==0.)
                self.assertEqual(row['parameters']['RefineGrid'],density<.65)
                counts.append(sum(m['faces'] for m in row['meshes']))
            self.assertEqual(counts,sorted(counts))
        self.assertEqual([m['faces'] for m in rows['plane-density100']['meshes']],[1])
        self.assertEqual([m['faces'] for m in rows['plane-analysis']['meshes']],[16])
        self.assertEqual(rows['sphere-analysis']['parameters']['RelativeTolerance'],.8)
        self.assertTrue(rows['sphere-analysis']['parameters']['RefineGrid'])

    def test_invalid_or_unbounded_recipes_are_rejected(self):
        base=request()['operations'][0]
        for change in [{'source':'Other'},{'profile':'Other'},{'id':'../escape'},{'extra':True},{'op':'Other'}]:
            with self.assertRaises(ValueError):validate(dict(base,**change))


if __name__=='__main__':unittest.main()
