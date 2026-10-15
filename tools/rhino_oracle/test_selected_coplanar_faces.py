"""Strict overload routing and ownership mocks, not licensed Rhino evidence."""
import copy
from pathlib import Path
import json
from unittest import TestCase, mock
from .selected_coplanar_faces_probe import request, validate, run
from . import test_coplanar_faces as helpers


class SelectedTests(TestCase):
    def test_strict_fields_indices_and_no_mesh_sdk_substitute(self):
        for op in request()['operations']: validate(op)
        base = request()['operations'][0]
        for update in [dict(extra=True), dict(region=1), dict(faces=[]), dict(faces=[0]),
                       dict(faces=[0, 0]), dict(faces=[0, True]), dict(faces=[-1, 1]),
                       dict(faces=[0, 6]), dict(faces=[0, 1, 2]), dict(op='merge_coplanar_faces')]:
            op = copy.deepcopy(base); op.update(update)
            with self.subTest(update=update), self.assertRaises(ValueError): validate(op)
        mesh = json.loads(Path(__file__).with_name('fixtures').joinpath('selected_coplanar_mesh_local.json').read_text())
        host = {'_object_source': mock.Mock()}
        for op in mesh['operations']:
            validate(op)
            with self.assertRaisesRegex(ValueError, 'getter capture remains pending'):
                run(op, host, 1, request()['tolerance'])
        host['_object_source'].assert_not_called()

    def test_selected_pair_and_seed_call_only_corresponding_sdk_overload(self):
        for region, indices in [(False, [0, 1]), (False, [1, 0]), (True, [1])]:
            host, source, output, area, _ = helpers.CoplanarTests().host()
            op = copy.deepcopy(request()['operations'][0]); op.update(region=region, faces=indices)
            value, _ = run(op, host, 1, request()['tolerance'])
            output.MergeCoplanarFaces.assert_called_once_with(*indices, 1e-9, 1e-8)
            self.assertEqual(value['removed_faces'], 1)
            for obj in (source, output, area): obj.Dispose.assert_called_once()

    def test_noop_and_all_failure_paths_release_owned_copies(self):
        for stage in ('noop', 'copy', 'native', 'invalid', 'mass', 'false_change', 'range'):
            host, source, output, area, _ = helpers.CoplanarTests().host()
            if stage == 'noop':
                output.Faces = source.Faces; output.MergeCoplanarFaces.return_value = False
            elif stage == 'copy': source.DuplicateBrep.return_value = None
            elif stage == 'native': output.MergeCoplanarFaces.side_effect = ValueError('SDK error')
            elif stage == 'invalid': output.IsValid = False
            elif stage == 'mass': host['Rhino'].Geometry.AreaMassProperties.Compute.return_value = None
            elif stage == 'false_change': output.MergeCoplanarFaces.return_value = False
            else: source.Faces = helpers.Faces(source.Faces[:1])
            with self.subTest(stage=stage):
                if stage == 'noop':
                    value, _ = run(request()['operations'][0], host, 1, request()['tolerance'])
                    self.assertEqual(value['removed_faces'], 0)
                else:
                    with self.assertRaises(ValueError):
                        run(request()['operations'][0], host, 1, request()['tolerance'])
                source.Dispose.assert_called_once()
                if stage not in ('copy', 'range'): output.Dispose.assert_called_once()
                else: output.Dispose.assert_not_called()
