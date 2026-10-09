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
