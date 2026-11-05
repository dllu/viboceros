"""Capture owned edge-analysis deletion and Undo under the private display wrapper."""
from .capture_family import capture
from .edge_analysis_lifetime_cases import runtime_cases
from .structural_sdk import PROBES

if __name__ == '__main__':
    modules = ['edge_analysis_lifetime_probe', *PROBES['edge_analysis_lifetime'][1]]
    capture('edge_analysis_lifetime_runtime',
            dict(protocol_version=1, iterations=1,
                 tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=runtime_cases()),
            ['generate_edge_analysis_lifetime_runtime.py', *[name+'.py' for name in modules],
             'structural_sdk.py', 'rhino_worker.py', 'client.py', 'fixtures/brep_direction/warped.3dm'],
            'VibocerosOracleEdgeLifetimeRuntime20261105', timeout=900)
