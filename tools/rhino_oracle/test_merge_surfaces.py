"""Owned SDK cleanup, immutable captures and finite support-replay gates."""
import copy
import gzip
import json
from pathlib import Path
import shutil
import tempfile
from types import SimpleNamespace as NS
import unittest
from unittest import mock
from . import merge_surfaces_evidence as evidence
from .merge_surface_cases import request
from . import merge_surfaces_sdk as sdk
from . import report_merge_surfaces as report


class SurfaceMergeTests(unittest.TestCase):
    def test_native_capture_inventory_and_frozen_sources(self):
        rows, captures, _ = evidence.collect()
        self.assertEqual((len(rows), len(captures)), (30, 3))
        self.assertEqual(sum(v['success'] for v in rows.values()), 28)
        self.assertFalse(rows['merge_bowed_boundary']['success'])
        self.assertFalse(rows['merge_large_gap']['success'])

    def test_recipe_admission(self):
        op = request(['merge_plane_sharp'])['operations'][0]
        sdk.validate(op)
        for changes in [dict(extra=1), dict(case='unknown'), dict(id=False), dict(op='merge')]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                sdk.validate(dict(op, **changes))

    def host(self):
        breps = [NS(name='first', changed=False, IsValid=True, Dispose=mock.Mock()),
                 NS(name='second', changed=False, IsValid=True, Dispose=mock.Mock())]
        sources = [NS(name='a', Dispose=mock.Mock(), ToBrep=mock.Mock(return_value=breps[0])),
                   NS(name='b', Dispose=mock.Mock(), ToBrep=mock.Mock(return_value=breps[1]))]
        output = NS(name='output', changed=False, IsValid=True, Dispose=mock.Mock())
        merge = mock.Mock(return_value=output)
        host = dict(Rhino=NS(Geometry=NS(Brep=NS(MergeSurfaces=merge), Point2d=NS(Unset=mock.sentinel.unset))),
                    _object_source=mock.Mock(side_effect=sources),
                    _nurbs_surface_definition=lambda s: dict(name=s.name))
        helper = NS(geometry_record=lambda b, *_: dict(name=b.name, changed=b.changed))
        return host, sources, breps, output, merge, helper

    def test_owned_call_parameters_and_native_decline(self):
        op = request(['merge_plane_sharp'])['operations'][0]
        tolerance = request([])['tolerance']
        for declined in [False, True]:
            host, sources, breps, output, merge, helper = self.host()
            if declined: merge.return_value = None
            with mock.patch.dict('sys.modules', {'brep_join_probe': helper}):
                value, _ = sdk.run(op, host, 1, tolerance)
            self.assertIs(value['success'], not declined)
            merge.assert_called_once_with(breps[0], breps[1], 1e-4, 1e-6,
                                          mock.sentinel.unset, mock.sentinel.unset, 1., False)
            for obj in sources + breps: obj.Dispose.assert_called_once()
            self.assertEqual(output.Dispose.call_count, int(not declined))

    def test_failure_cleanup(self):
        for stage in ['input', 'native', 'invalid', 'mutation']:
            host, sources, breps, output, merge, helper = self.host()
            if stage == 'input': sources[0].ToBrep.return_value = None
            elif stage == 'native': merge.side_effect = ValueError('kernel failure')
            elif stage == 'invalid': output.IsValid = False
            else:
                def changed(*_):
                    breps[0].changed = True
                    return output
                merge.side_effect = changed
            with self.subTest(stage=stage), mock.patch.dict('sys.modules', {'brep_join_probe': helper}), self.assertRaises(ValueError):
                sdk.run(request(['merge_plane_sharp'])['operations'][0], host, 1, request([])['tolerance'])
            sources[0].Dispose.assert_called_once()
            if stage != 'input':
                for obj in sources[1:] + breps: obj.Dispose.assert_called_once()
            self.assertEqual(output.Dispose.call_count, int(stage in ['invalid', 'mutation']))

    def test_bad_capture_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(evidence.PROOF, proof)
            path = proof / 'captures.json'
            receipts = json.loads(path.read_bytes())
            for key, value in [('exit_status', 1), ('command', ['desktop-launcher'])]:
                changed = copy.deepcopy(receipts)
                changed[0][key] = value
                path.write_text(json.dumps(changed))
                with self.assertRaisesRegex(ValueError, 'headless capture receipt'):
                    evidence.collect(proof)

    def test_missing_duplicate_and_nonfinite_replays_are_rejected(self):
        if not (evidence.PROOF / 'kernel-native.log.gz').exists():
            self.skipTest('replay follows capture')
        with tempfile.TemporaryDirectory() as directory:
            proof = Path(directory) / 'proof'
            shutil.copytree(evidence.PROOF, proof)
            path = proof / 'kernel-native.log.gz'
            lines = gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index = next(i for i, line in enumerate(lines) if line.startswith('MERGE_SURFACE_SUPPORT '))
            for changed in [lines[:index] + lines[index + 1:], lines + [lines[index]]]:
                path.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'incomplete or duplicate'):
                    report.comparisons(proof)
            row = json.loads(lines[index].split(' ', 1)[1])
            for error in [float('nan'), float('inf'), 1., True]:
                changed = lines.copy()
                changed[index] = 'MERGE_SURFACE_SUPPORT ' + json.dumps(dict(row, maximum_geometry_error=error)) + '\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(), mtime=0))
                with self.assertRaisesRegex(ValueError, 'finite geometry gate'):
                    report.comparisons(proof)

    def test_record_reproduces(self):
        path = evidence.ROOT / 'docs/merge-surfaces-validation.json'
        if not path.exists(): self.skipTest('record follows implementation')
        value = json.loads(path.read_bytes())
        self.assertEqual(value, report.build(evidence.PROOF, value['implementation_revision']))


if __name__ == '__main__': unittest.main()
