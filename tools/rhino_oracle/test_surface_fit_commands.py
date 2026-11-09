"""Owned command schemas, staging, source constructors and failed launches."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from . import surface_fit_command_probe as probe
from . import generate_surface_fit_commands as pilot
from . import generate_surface_fit_command_faces as faces
from .structural_sdk import stage_helpers,validate_operations


class SurfaceFitCommandTests(unittest.TestCase):
    def test_requests_reproduce_frozen_inputs_and_stage_standalone_helpers(self):
        for family,module,count in [('surface_fit_commands',pilot,8),('surface_fit_command_faces',faces,7)]:
            query=module.request();before=copy.deepcopy(query);validate_operations(query)
            self.assertEqual(query,json.loads((Path(__file__).parent/'fixtures'/(family+'.json')).read_bytes()))
            self.assertEqual(len(query['operations']),count)
            with tempfile.TemporaryDirectory()as directory:
                stage_helpers(query,Path(__file__).parent,Path(directory))
                self.assertEqual(sorted(p.name for p in Path(directory).iterdir()),['merge_edges_probe.py','surface_fit_command_probe.py'])
            self.assertEqual(query,before)
        # Preserve the actual source behind the historically misleading case label.
        operations=faces.request()['operations']
        self.assertEqual(operations[4]['surface'],operations[0]['surface'])

    def test_schema_rejects_unbounded_or_ambiguous_macro_inputs(self):
        op=pilot.request()['operations'][0]
        for change in [dict(delete_input=1),dict(preselect='yes'),dict(degree_u=True),dict(degree_v=12),
                       dict(fit_tolerance=-1.),dict(fit_tolerance=float('inf')),dict(output_layer='target'),
                       dict(construction='other'),dict(extra=True)]:
            with self.assertRaises(ValueError):probe.validate(dict(op,**change))
        op=copy.deepcopy(op);op['surface']['control_point_count_u']=65
        with self.assertRaises(ValueError):probe.validate(op)

    def test_both_failed_single_face_assumptions_keep_exact_producer_closures(self):
        root=Path(__file__).parent;failed=list((root/'observations/failures').glob('surface_fit_commands-*.json.gz'))
        self.assertEqual(len(failed),2)
        for path in failed:
            capsule=json.loads(gzip.decompress(path.read_bytes()))
            self.assertIn('unexpected FitSrf',capsule['error'])
            for sha in capsule['provenance']['sources'].values():
                data=gzip.decompress((root/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)


if __name__=='__main__':unittest.main()
