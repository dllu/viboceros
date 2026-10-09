"""Bounded, finite Contour fixtures are checked before launching Rhino."""
import unittest
from .contour_probe import validate
class ContourProbeTests(unittest.TestCase):
    def fixture(self):
        return dict(op='contour_command',id='case',sources=[dict(type='line',start=[-2,0,0],end=[2,0,0])],start=[0,0,0],end=[1,0,0],spacing=1)
    def test_spacing_points_options_and_source_count_are_validated(self):
        for field,value in [('spacing',0),('spacing',-1),('spacing',True),('spacing',float('inf')),('spacing',float('nan')),('range',1),('group','yes'),('properties','invalid'),('start',[0,0]),('end',[float('inf'),0,0]),('sources',[]),('sources',[{}]*65),('unknown',1)]:
            f=self.fixture();f[field]=value
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):validate(f)
    def test_valid_fixture_can_include_range_and_a_rotated_cplane(self):
        f=self.fixture();f.update(range=True,group=True,properties='input',x_axis=[0,1,0],y_axis=[0,0,1]);validate(f)
    def test_ineligible_sources_are_rejected_before_a_native_command_can_wait_for_picks(self):
        for sources in [[dict(type='point',point=[0,0,0])],[dict(type='point_cloud',points=[[0,0,0]])],[1]]:
            f=self.fixture();f['sources']=sources
            with self.assertRaises(ValueError):validate(f)
    def test_live_commands_require_owned_settings_and_a_single_iteration(self):
        import os,tempfile
        from unittest.mock import patch
        from .client import OracleClient,OracleProtocolError
        with tempfile.NamedTemporaryFile() as launcher,patch.dict(os.environ,{'DISPLAY':':198','VIBOCEROS_ORACLE_HEADLESS':':198'}):
            for scheme,iterations in [(None,1),('VibocerosOracleContourValidation',2),('VibocerosOracleContourValidation',True)]:
                client=OracleClient(launcher=launcher.name,settings_scheme=scheme)
                request=dict(protocol_version=1,iterations=iterations,operations=[self.fixture()])
                with self.subTest(scheme=scheme,iterations=iterations),self.assertRaises(OracleProtocolError):client.run_rhino(request)
