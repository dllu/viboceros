"""Immutable open-chain captures and the exact command's qualified scope."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import planar_chains_cases as cases
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(planar_chains=cases.pilot, planar_chains_holdouts=cases.holdouts,
                planar_chains_contexts=cases.contexts, planar_chains_salvage=cases.salvage,
                planar_chains_filled=cases.filled, planar_chains_modes=cases.modes,
                planar_chain_networks=cases.networks, planar_chain_network_holdouts=cases.network_holdouts,
                planar_network_crossings=cases.network_crossings)

class PlanarChainsTests(unittest.TestCase):
    def test_requests_and_frozen_sources(self):
        count = 0
        for family, make in FAMILIES.items():
            path = HERE / ('fixtures/' + family + '.json')
            query = json.loads(path.read_bytes()); native = read(family)
            self.assertEqual(query['operations'], make()); validate_operations(query)
            self.assertEqual([r['id'] for r in native['results']], [r['id'] for r in make()])
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, sha in native['provenance']['sources'].items():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())).hexdigest(), sha, name)
            count += len(make())
        self.assertEqual(count, 198)

    def test_native_source_preservation_history_and_valid_outputs(self):
        failed = []
        for family in FAMILIES:
            for row in read(family)['results']:
                v = row['value']; self.assertEqual(v['created'], v['after'])
                self.assertEqual(v['before'], v['undo']); self.assertEqual(v['after'], v['redo'])
                self.assertEqual(v['before'], [o for o in v['after'] if o['source'] is not None])
                outputs = [o for o in v['after'] if o['kind'] == 'brep']
                if not v['success']: failed.append(row['id']); self.assertFalse(outputs)
                for out in outputs:
                    self.assertTrue(out['native_valid'], row['id']); self.assertEqual(out['selected'], 0)
                    self.assertEqual(out['faces'], 1); self.assertEqual(out['loops'][0]['kind'], 'Outer')
        self.assertEqual(failed, ['chains_incomplete', 'chains_single_line', 'chains_nonplanar',
            'chains_holdout_gap_y_1_01', 'chains_holdout_gap_y_1_5', 'chains_holdout_gap_y_2_1',
            'chains_holdout_gap_y_2_11', 'chains_holdout_gap_y_3_0',
            'chains_holdout_gap_xy_1_0', 'chains_holdout_gap_xy_1_01', 'chains_holdout_gap_xy_1_5',
            'chains_holdout_gap_xy_2_1', 'chains_holdout_gap_xy_2_11', 'chains_holdout_gap_xy_3_0'])

    def test_validator_rejects_modified_owned_recipes(self):
        row = cases.pilot()[0]
        for mutate in [lambda r:r.update(extra=True), lambda r:r.update(id='unowned'),
                       lambda r:r['curves'][0]['control_points'][0]['point'].__setitem__(0, True),
                       lambda r:r['curves'][0]['knots'].__setitem__(0, 1.)]:
            changed = copy.deepcopy(row); mutate(changed)
            with self.assertRaises(ValueError): cases.validate(changed)

    def test_probe_imports_in_isolation(self):
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(dict(operations=[cases.pilot()[0]]), HERE, Path(directory))
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import planar_chains_probe', directory], check=True, capture_output=True)

if __name__ == '__main__': unittest.main()
