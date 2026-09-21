"""Selected-edge API validation and owned artifact isolation."""
import copy
import math
import unittest
import sys
from types import SimpleNamespace
from unittest.mock import patch
from pathlib import Path
from . import merge_edge_probe
from .client import _owned_artifact_request, OracleProtocolError


class SelectedEdgeMergeTests(unittest.TestCase):
    def run_owned_probe(self, failure=None):
        owned, models, paths = [], [], []
        class Brep:
            def __init__(self):
                self.IsValid, self.disposed = failure != 'invalid-source', 0
                self.Edges = SimpleNamespace(Count=6, MergeEdge=self.merge)
                owned.append(self)
            def Duplicate(self):
                if failure == 'duplicate': raise ValueError('duplicate failed')
                return Brep()
            def DuplicateBrep(self):
                result = Brep()
                result.Edges.Count, result.IsValid = self.Edges.Count, self.IsValid
                return result
            def Dispose(self): self.disposed += 1
            def merge(self, edge, angle):
                if failure == 'merge': raise ValueError('merge failed')
                return 123  # Keep API return independent of the topology count.
            def Compact(self):
                if failure == 'compact': raise ValueError('compact failed')
                self.Edges.Count = 3
                self.IsValid = failure != 'invalid-output'
        class Model:
            def __init__(self):
                self.geometry, self.disposed = Brep(), 0
                self.Objects = [] if failure == 'empty' else [SimpleNamespace(Geometry=self.geometry)]
                models.append(self)
            def Dispose(self):
                self.disposed += 1
                self.geometry.Dispose()
        def read(path):
            paths.append(path)
            return None if failure == 'read' else Model()
        def record(brep, tolerance, host):
            if failure == 'record': raise ValueError('record failed')
            return dict(edges=[0]*brep.Edges.Count)
        host = dict(Rhino=SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep),
            FileIO=SimpleNamespace(File3dm=SimpleNamespace(Read=read))))
        op = dict(source=dict(artifact_path='/owned/model.3dm'), edge=6 if failure == 'edge' else 0, angle=0.1)
        with patch.dict(sys.modules, dict(brep_join_probe=SimpleNamespace(geometry_record=record))):
            if failure:
                with self.assertRaises(ValueError): merge_edge_probe.run(op, None, host)
            else:
                value, elapsed = merge_edge_probe.run(op, None, host)
                self.assertEqual(value, dict(before=dict(edges=[0]*6), after=dict(edges=[0]*3), removed=3, api_return=123))
                self.assertEqual(elapsed, 0)
        self.assertEqual(paths, ['Z:\\owned\\model.3dm'])
        self.assertTrue(all(x.disposed == 1 for x in owned + models))

    def test_owned_model_and_copy_are_disposed_after_measurement(self):
        self.run_owned_probe()

    def test_owned_geometry_is_disposed_on_each_failure_path(self):
        for failure in ['read', 'empty', 'duplicate', 'invalid-source', 'edge', 'merge', 'compact', 'invalid-output', 'record']:
            with self.subTest(failure=failure): self.run_owned_probe(failure)

    def test_validation_precedes_host_access(self):
        valid = dict(source=dict(artifact_path='/owned/model.3dm'), edge=0, angle=0)
        updates = [dict(source=x) for x in (None, [], {}, dict(artifact_path=''), dict(artifact_path=1))]
        updates += [dict(edge=x) for x in (None, True, -1, 1.5, '0')]
        updates += [dict(angle=x) for x in (None, True, -1, math.inf, math.nan, '1', math.pi+1e-6)]
        for update in updates:
            with self.subTest(update=update), self.assertRaises(ValueError):
                merge_edge_probe.run(dict(valid, **update), None, {})
        for angle in (0, 1e-10, math.pi):
            self.assertEqual(merge_edge_probe.validate(dict(valid, angle=angle)), ('/owned/model.3dm', 0, angle))

    def test_artifacts_are_unique_owned_and_never_overwrite_caller_paths(self):
        request = {'operations': [dict(op='brep_merge_edge', id='../../untrusted',
            source=dict(source={'type':'box'}, artifact_path='/unowned/model.3dm'), edge=1, angle=0)] * 2}
        original = copy.deepcopy(request)
        with _owned_artifact_request(request) as prepared:
            paths = [Path(o['source']['artifact_path']) for o in prepared['operations']]
            self.assertEqual(len(set(paths)), 2)
            self.assertTrue(all(p.parent.is_dir() and not p.exists() for p in paths))
            self.assertEqual([o['source']['source'] for o in prepared['operations']], [{'type':'box'}]*2)
        self.assertEqual(request, original)
        self.assertTrue(all(not p.parent.exists() for p in paths))

    def test_artifact_setup_rejects_missing_sources(self):
        for source in (None, [], 'source'):
            with self.subTest(source=source), self.assertRaises(OracleProtocolError):
                with _owned_artifact_request({'operations':[dict(op='brep_merge_edge', source=source)]}):
                    pass
