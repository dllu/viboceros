"""Native public length tolerances and automatic curve parameterization."""
from .capture_family import capture
if __name__=='__main__':
    capture('revolve_domain_estimates',dict(protocol_version=1,iterations=1,
        operations=[dict(op='revolve_domain_sampling',id=name,case=name)for name in ['line','rational','spatial','axis_tips']]),
        ['generate_revolve_domain_estimates.py','revolve_domain_sampling_probe.py','revolve_parameterization_probe.py','revolve_deformable_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveDomainEstimates20261106',timeout=900)
