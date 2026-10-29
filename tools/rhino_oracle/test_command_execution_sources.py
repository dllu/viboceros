"""Extracted command dispatch stays inside native source qualification."""
import unittest
from .qualification_sources import with_shared_sources


class CommandExecutionSourcesTests(unittest.TestCase):
    def test_command_root_and_dispatch_are_bound_from_either_entry(self):
        entries = ['crates/viboceros-command/src/lib.rs',
                   'crates/viboceros-command/src/execution.rs']
        for entry in entries:
            expanded = with_shared_sources([entry])
            self.assertTrue(set(entries) <= set(expanded))
            self.assertEqual(with_shared_sources(expanded), expanded)
            self.assertEqual(len(expanded), len(set(expanded)))
