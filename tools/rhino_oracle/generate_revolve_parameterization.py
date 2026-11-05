"""Record public revolution domain queries in an isolated headless session."""
from .capture_family import capture

if __name__=='__main__':
    capture('revolve_parameterization',dict(protocol_version=1,iterations=1,
        operations=[dict(op='revolve_parameterization',id=name,case=name)for name in ['line','rational','spatial','axis_tips']]),
        ['generate_revolve_parameterization.py','revolve_parameterization_probe.py','revolve_deformable_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveDomains20261106',timeout=900)
