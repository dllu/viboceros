"""JoinBreps uses only owned geometry, including failed probes."""
import types
import unittest
from unittest.mock import patch
from . import brep_join_probe as probe


class BrepJoinProbeTests(unittest.TestCase):
    def test_validation_precedes_all_host_access(self):
        for paths in (None, [], "source.3dm", [1], [""], [False]):
            with self.subTest(paths=paths), self.assertRaises(ValueError):
                probe.run({"artifact_paths": paths, "join_tolerance": 0.001}, None, {})
        for tolerance in (None, False, "0.001", -1, float("nan"), float("inf")):
            with self.subTest(tolerance=tolerance), self.assertRaises(ValueError):
                probe.run({"artifact_paths": ["a.3dm"], "join_tolerance": tolerance}, None, {})
        probe.validate({"artifact_paths": ["a.3dm"], "join_tolerance": 0})

    def run_fake(self, failure=None):
        created, models = [], []
        class Brep:
            def __init__(self):
                self.IsValid, self.value, self.disposed = True, 1, 0
                created.append(self)
            def Duplicate(self): return Brep()
            def Dispose(self): self.disposed += 1
            @staticmethod
            def JoinBreps(inputs, tolerance):
                self.assertEqual(tolerance, 0.001)
                if failure == "join": raise ValueError("join failed")
                result = Brep()
                if failure == "invalid": result.IsValid = False
                if failure == "mutation": inputs[0].value = 2
                return [result]
        class Model:
            def __init__(self):
                self.disposed = 0
                # Artifact-owned originals are disposed with the model.
                self.geometry = Brep()
                self.Objects = [types.SimpleNamespace(Geometry=self.geometry)]
                if failure == "empty": self.Objects = []
                models.append(self)
            def Dispose(self):
                self.disposed += 1
                self.geometry.Dispose()
        host = {"Rhino": types.SimpleNamespace(
            FileIO=types.SimpleNamespace(File3dm=types.SimpleNamespace(Read=lambda path: Model())),
            Geometry=types.SimpleNamespace(Brep=Brep))}
        with patch.object(probe, "geometry_record", side_effect=lambda g, t, h: {"value": g.value}):
            operation = {"artifact_paths": ["a.3dm", "b.3dm"], "join_tolerance": 0.001}
            if failure:
                with self.assertRaises(ValueError): probe.run(operation, None, host)
            else:
                value, elapsed = probe.run(operation, None, host)
                self.assertEqual(value, {"inputs": [{"value": 1}, {"value": 1}], "outputs": [{"value": 1}]})
                self.assertEqual(elapsed, 0)
        self.assertTrue(created)
        self.assertTrue(all(g.disposed == 1 for g in created))
        self.assertTrue(all(m.disposed == 1 for m in models))

    def test_owned_inputs_outputs_and_models_are_disposed_on_success(self):
        self.run_fake()

    def test_cleanup_survives_import_join_validation_and_mutation_errors(self):
        for failure in ("empty", "join", "invalid", "mutation"):
            with self.subTest(failure=failure): self.run_fake(failure)
