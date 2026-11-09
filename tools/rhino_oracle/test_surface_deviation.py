import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from .report_surface_deviation import ROOT, PROOF, build


class SurfaceDeviationEvidenceTests(unittest.TestCase):
    def test_record_reproduces_complete_backend_scope(self):
        value = build()
        self.assertEqual(value, json.loads((ROOT / 'docs/surface-deviation-validation.json').read_bytes()))
        self.assertEqual({key: row['full_surface_bounds'] for key, row in value['backends'].items()},
                         {'portable': 476, 'native': 476})
        self.assertFalse(value['averaged_refinement_qualified'])

    def test_missing_sources_failed_commands_and_dropped_cases_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(PROOF, proof)
            path = proof / 'sources.json'
            original = path.read_bytes()
            value = json.loads(original)
            value['overrides'].pop(next(iter(value['overrides'])))
            path.write_text(json.dumps(value))
            with self.assertRaises(ValueError):
                build(proof)
            path.write_bytes(original)
            path = proof / 'checks.json'
            original = path.read_bytes()
            value = json.loads(original)
            for field, bad in [('exit_status', 101), ('command', ['true'])]:
                changed = copy.deepcopy(value)
                changed[-1][field] = bad
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):
                    build(proof)
            path.write_bytes(original)
            path = proof / 'native.log.gz'
            text = gzip.decompress(path.read_bytes()).decode().splitlines()
            index = next(i for i, line in enumerate(text) if line.startswith('SURFACE_PAIR_CERTIFICATE '))
            path.write_bytes(gzip.compress('\n'.join(text[:index] + text[index + 1:]).encode(), mtime=0))
            with self.assertRaises(ValueError):
                build(proof)


if __name__ == '__main__':
    unittest.main()
