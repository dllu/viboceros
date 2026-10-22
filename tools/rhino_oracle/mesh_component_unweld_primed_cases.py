"""Bounded normal-option setup before native component preselection."""
from copy import deepcopy
try:
    from .mesh_component_unweld_command_cases import CASES as COMMANDS
except (ImportError, ValueError):
    from mesh_component_unweld_command_cases import CASES as COMMANDS
CASES={}
for kind in ['edge','vertex']:
    for name in ['bent','cube','no_normals']:
        for policy in ['preserve','modify']:
            original='command_'+kind+'_'+name+'_'+policy
            case=deepcopy(COMMANDS[original]);case['prime_options']=True
            CASES['primed_'+kind+'_'+name+'_'+policy]=case


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_unweld_commands',id=name,case=name) for name in CASES])
