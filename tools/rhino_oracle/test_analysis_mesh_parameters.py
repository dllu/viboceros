"""Immutable public native settings, mesh counts, limit samples and archive reads."""
import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from .capture_family import HERE
from . import generate_analysis_mesh_parameters as parameters
from . import generate_analysis_subd_details as details
from .analysis_mesh_parameters_probe import validate as validate_parameters
from .analysis_subd_details_probe import validate as validate_details, ARCHIVES
from .structural_sdk import stage_helpers


class AnalysisMeshParametersTests(unittest.TestCase):
    def test_native_capture_provenance_and_coverage(self):
        for family, generator in [('analysis_mesh_parameters', parameters), ('analysis_subd_details', details)]:
            fixture = HERE / ('fixtures/'+family+'.json')
            request = json.loads(fixture.read_text())
            native = json.loads((HERE / ('observations/'+family+'.json')).read_text())
            self.assertEqual(request, generator.request())
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in request['operations']])
            for name, sha in native['provenance']['sources'].items():
                data = gzip.decompress((HERE / ('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha)
                if name.endswith('.3dm'): self.assertEqual(data, (HERE / name).read_bytes())
            self.assertEqual(native['engine'], 'rhino')
            self.assertNotIn('error', native['results'][0])

    def test_native_default_extraction_and_saved_document_settings(self):
        native = json.loads((HERE / 'observations/analysis_mesh_parameters.json').read_text())['results'][0]['value']
        self.assertEqual(native['current'], native['default'])
        self.assertEqual(native['default']['subd_level'], 4)
        self.assertTrue(native['default']['subd_adaptive'])
        self.assertEqual(native['adaptive_quad_maximum'], 512000)
        self.assertEqual(len(native['mappings']), 7)
        self.assertEqual(len(native['meshes']), 4)
        for source in native['meshes']:
            self.assertEqual([r['level'] for r in source['levels']], list(range(1,6)))
            self.assertEqual(source['levels'][3]['mesh'], source['extraction'])
        native = json.loads((HERE / 'observations/analysis_subd_details.json').read_text())['results'][0]['value']
        self.assertEqual(len(native['limits']), 24)
        self.assertEqual([r['name'] for r in native['archives']], list(ARCHIVES))
        for row in native['archives']:
            level, mode = row['name'].split('-')
            level = int(level[5:]); adaptive = mode == 'adaptive'
            self.assertEqual(row['adaptive'], adaptive)
            self.assertEqual(row['effective_empty'], max(1,level) if adaptive else level)

    def test_staging_keeps_archive_inputs_inside_the_owned_job(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            stage_helpers(details.request(), HERE, destination)
            for name in ARCHIVES:
                asset = Path('fixtures/analysis_subd_settings') / (name+'.3dm')
                self.assertEqual((destination / asset).read_bytes(), (HERE / asset).read_bytes())

    def test_fixed_recipes_reject_extra_or_unbounded_input(self):
        for request, validator in [(parameters.request(),validate_parameters),(details.request(),validate_details)]:
            op = request['operations'][0]
            for changes in [{'id':'../escape'},{'extra':True},{'op':'Other'}]:
                with self.assertRaises(ValueError): validator(dict(op, **changes))


if __name__ == '__main__': unittest.main()
