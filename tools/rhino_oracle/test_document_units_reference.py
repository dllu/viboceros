"""Lifecycle checks for the isolated Rhino public-API unit probe."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock, patch


class DocumentUnitsReferenceTests(unittest.TestCase):
    def load_probe(self, rhino, system):
        path = Path(__file__).with_name("generate_document_units_reference.py")
        spec = importlib.util.spec_from_file_location("unit_probe_under_test", path)
        module = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"Rhino": rhino, "System": system}):
            spec.loader.exec_module(module)
        return module

    def test_failed_object_add_disposes_attributes_and_document(self):
        rhino, system = MagicMock(), MagicMock()
        document = rhino.RhinoDoc.CreateHeadless.return_value
        attributes = rhino.DocObjects.ObjectAttributes.return_value
        document.Objects.AddPoint.side_effect = RuntimeError("add failed")
        probe = self.load_probe(rhino, system)
        with self.assertRaisesRegex(RuntimeError, "add failed"):
            probe.generate()
        attributes.Dispose.assert_called_once_with()
        document.Dispose.assert_called_once_with()

    def test_missing_probe_object_is_not_reported_as_success(self):
        rhino, system = MagicMock(), MagicMock()
        system.Guid.Empty = 0
        document = rhino.RhinoDoc.CreateHeadless.return_value
        document.Objects.AddPoint.side_effect = [1, 2, 3]
        document.Objects.FindId.return_value = None
        probe = self.load_probe(rhino, system)
        with self.assertRaisesRegex(ValueError, "lost an object"):
            probe.generate()
        self.assertEqual(rhino.DocObjects.ObjectAttributes.return_value.Dispose.call_count, 3)
        document.Dispose.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
