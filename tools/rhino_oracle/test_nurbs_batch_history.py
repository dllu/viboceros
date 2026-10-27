"""Keep numerical history measurements isolated from precise batch queries."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase
from .arc_length_batch_probe import _numerical_batches, validate
from .generate_nurbs_batch_history import request


class NurbsBatchHistoryTests(TestCase):
    def test_native_captures_bind_the_recipe_and_exact_frozen_producers(self):
        from . import generate_nurbs_batch_boundaries as boundaries
        from . import generate_nurbs_batch_history as history
        from . import generate_nurbs_batch_resolution as resolution
        from . import generate_nurbs_batch_holdouts as holdouts
        root = Path(__file__).parent
        for family, producer, count in [('nurbs_batch_boundaries', boundaries, 64),
                ('nurbs_batch_history', history, 6),
                ('nurbs_batch_resolution', resolution, 34),
                ('nurbs_batch_holdouts', holdouts, 72)]:
            fixture = (root / 'fixtures' / (family + '.json')).read_bytes()
            query = json.loads(fixture)
            record = json.loads((root / 'observations' / (family + '.json')).read_text())
            self.assertEqual(query, producer.request())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([op['id'] for op in query['operations']],
                             [row['id'] for row in record['results']])
            receipt = record['provenance']
            self.assertEqual(hashlib.sha256(fixture).hexdigest(), receipt['request_sha256'])
            for digest in receipt['sources'].values():
                archive = root / 'observations/source_snapshots' / (digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)

    def test_saved_native_batches_are_independent_of_the_measured_query_history(self):
        root = Path(__file__).parent
        native = json.loads((root / 'observations/nurbs_batch_history.json').read_text())
        for row in native['results']:
            scenarios = request()['operations'][0]['inspect_numerical_batch']
            batches = row['value']['numerical_batches']
            cold = {s['fractional_tolerance']: b['parameters']
                    for s, b in zip(scenarios, batches) if not s['history']}
            for scenario, batch in zip(scenarios, batches):
                self.assertEqual(batch['parameters'], cold[scenario['fractional_tolerance']])
                self.assertEqual(batch['before'], batch['after'])

    def test_bounded_scenarios_and_explicit_query_history(self):
        query = request()
        self.assertEqual(len(query['operations']), 6)
        for operation in query['operations']:
            validate(operation)
        original = query['operations'][0]
        for value in [None, [], [None], original['inspect_numerical_batch'] * 2]:
            changed = copy.deepcopy(original)
            changed['inspect_numerical_batch'] = value
            with self.assertRaises(ValueError):
                validate(changed)
        for key, value in [('id', 'bad\nname'), ('fractional_tolerance', True),
                           ('fractional_tolerance', float('nan')),
                           ('fractional_tolerance', 0), ('fractional_tolerance', 1),
                           ('history', [dict(query='unknown', fractional_tolerance=.01)]),
                           ('history', [dict(query='length', fractional_tolerance=.01)] * 9)]:
            changed = copy.deepcopy(original)
            changed['inspect_numerical_batch'][0][key] = value
            with self.assertRaises(ValueError, msg=key):
                validate(changed)

    def test_each_scenario_owns_a_fresh_curve_and_no_implicit_length_query(self):
        created = []

        class Curve:
            def __init__(self):
                self.queries = []
                self.disposed = False
                created.append(self)

            def GetLength(self, accuracy):
                self.queries.append(('length', accuracy))
                return 2.

            def NormalizedLengthParameter(self, fraction, accuracy):
                self.queries.append(('scalar', accuracy))
                assert fraction == .5
                return True, 1.

            def NormalizedLengthParameters(self, fractions, absolute, accuracy):
                self.queries.append(('batch', accuracy))
                assert absolute == 0.
                return [2. * f for f in fractions]

            def PointAt(self, parameter):
                return [parameter, 0., 0.]

            def Dispose(self):
                self.disposed = True

        class Arrays:
            def __getitem__(self, _):
                return list

        operation = request()['operations'][0]
        host = dict(_object_source=lambda definition, tolerance: Curve(),
            System=SimpleNamespace(Array=Arrays(), Double=float), _xyz=lambda point: point,
            _nurbs_curve_definition=lambda curve: copy.deepcopy(operation['curve']))
        records = _numerical_batches(operation, host, {})
        self.assertEqual(len(records), 14)
        self.assertEqual(len(created), 14)
        for scenario, curve, record in zip(operation['inspect_numerical_batch'], created, records):
            expected = [(q['query'], q['fractional_tolerance']) for q in scenario['history']]
            expected.append(('batch', scenario['fractional_tolerance']))
            self.assertEqual(curve.queries, expected)
            self.assertTrue(curve.disposed)
            self.assertEqual(record['before'], record['after'])
            self.assertEqual(record['parameters'], [i / 4. for i in range(9)])

    def test_curve_is_disposed_when_a_public_query_fails(self):
        class Broken:
            disposed = False
            def NormalizedLengthParameters(self, *args):
                return None
            def Dispose(self):
                self.disposed = True
        class Arrays:
            def __getitem__(self, _):
                return list
        curve = Broken()
        operation = request()['operations'][0]
        host = dict(_object_source=lambda definition, tolerance: curve,
            System=SimpleNamespace(Array=Arrays(), Double=float),
            _nurbs_curve_definition=lambda curve: operation['curve'])
        with self.assertRaises(ValueError):
            _numerical_batches(operation, host, {})
        self.assertTrue(curve.disposed)
