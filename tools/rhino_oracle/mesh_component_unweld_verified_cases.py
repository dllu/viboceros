"""Full command corpus with normal options set before component preselection."""
from copy import deepcopy
try:
    from .mesh_component_unweld_command_cases import CASES as COMMANDS
except (ImportError, ValueError):
    from mesh_component_unweld_command_cases import CASES as COMMANDS
CASES={}
for name,source in COMMANDS.items():
    case=deepcopy(source);case['prime_options']=True
    CASES[name.replace('command_','verified_',1)]=case


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_component_unweld_commands',id=name,case=name) for name in CASES])
