import unittest
from .divide_preferences_probe import SPECS, validate


class DividePreferencesProbeTests(unittest.TestCase):

    def test_known_bounded_cases_are_valid(self):
        for case in SPECS:
            validate(dict(op='divide_preferences', id=case, case=case))

    def test_unknown_fields_cases_and_invalid_ids_are_rejected(self):
        for update in [
            dict(case='unknown'), dict(id=''), dict(id='x' * 81),
            dict(extra=True), dict(op='divide_command'), dict(case=[]),
            dict(case={}), dict(id=42), dict(id='line\n'),
        ]:
            operation = dict(op='divide_preferences', id='memory', case='count_repeat')
            operation.update(update)
            with self.subTest(update=update), self.assertRaises(ValueError):
                validate(operation)
