"""Edge-analysis source admission is checked before starting Rhino."""
import unittest

from .edge_analysis_probe import validate


class EdgeAnalysisProbeTests(unittest.TestCase):
    def fixture(self):
        return dict(op='edge_analysis', id='box', sources=[
            dict(type='box_brep', min=[0, 0, 0], max=[1, 2, 3])])

    def test_rejects_empty_excessive_and_ineligible_sources(self):
        for sources in ([], [{}] * 65, [1], [dict(type='point')],
                        [dict(type='line')], [dict(type='unknown')]):
            fixture = self.fixture()
            fixture['sources'] = sources
            with self.subTest(sources=sources), self.assertRaises(ValueError):
                validate(fixture)

    def test_rejects_missing_and_unknown_fields(self):
        fixture = self.fixture()
        fixture['show'] = 'All'
        with self.assertRaises(ValueError):
            validate(fixture)
        del fixture['show']
        del fixture['sources']
        with self.assertRaises(ValueError):
            validate(fixture)

    def test_accepts_supported_geometry_sources(self):
        fixture = self.fixture()
        for kind in ('box_brep', 'brep', 'surface', 'mesh'):
            fixture['sources'][0]['type'] = kind
            validate(fixture)

    def test_workflow_actions_are_bounded_and_cannot_inject_native_commands(self):
        fixture = self.fixture()
        fixture['workflow'] = dict(command='ZoomNaked', actions=['Next', 'Mark', 'Mark'])
        validate(fixture)
        for field, value in [('command', 'Quit'), ('actions', ['_Enter']),
                             ('actions', 'Mark'), ('actions', ['Next'] * 33),
                             ('undo_redo', 1), ('unknown', True)]:
            workflow = dict(command='ZoomNaked', actions=['Mark'])
            workflow[field] = value
            fixture['workflow'] = workflow
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate(fixture)

    def test_live_workflows_require_private_settings_and_one_iteration(self):
        import os
        import tempfile
        from unittest.mock import patch
        from .client import OracleClient, OracleProtocolError
        fixture = self.fixture()
        fixture['workflow'] = dict(command='ZoomNaked', actions=['Mark'])
        with tempfile.NamedTemporaryFile() as launcher, patch.dict(os.environ, {
                'DISPLAY': ':198', 'VIBOCEROS_ORACLE_HEADLESS': ':198'}):
            for scheme, iterations in [(None, 1), ('VibocerosOracleEdgePreflight', 2),
                                       ('VibocerosOracleEdgePreflight', True)]:
                client = OracleClient(launcher=launcher.name, settings_scheme=scheme)
                request = dict(protocol_version=1, iterations=iterations, operations=[fixture])
                with self.subTest(scheme=scheme, iterations=iterations), self.assertRaises(OracleProtocolError):
                    client.run_rhino(request)
