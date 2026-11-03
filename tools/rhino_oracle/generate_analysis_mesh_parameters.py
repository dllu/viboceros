"""Freeze public native SubD analysis density rules and limit mesh samples."""
from .capture_family import capture
from .analysis_mesh_parameters_probe import validate


def request():
    op = dict(op='analysis_mesh_parameters', id='parameters')
    validate(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-8, relative=1e-12, angular=1e-8), operations=[op])


def main():
    capture('analysis_mesh_parameters', request(),
            ['generate_analysis_mesh_parameters.py', 'analysis_mesh_parameters_probe.py',
             'client.py', 'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py'],
            'VibocerosOracleAnalysisParameters20261102', timeout=900)


if __name__ == '__main__':
    main()
