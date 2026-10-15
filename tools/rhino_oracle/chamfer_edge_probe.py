"""Public SDK ChamferEdge recipes; licensed capture remains pending."""
import json
from pathlib import Path
try:
    from . import edge_blend_probe as shared
except ImportError:
    import edge_blend_probe as shared

def validate(operation):
    shared.validate(operation, 'chamfer_edge', 'distance', 'ChamferEdge')

def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('chamfer_edge.json').read_text())

def run(operation, host, iterations, tolerance):
    validate(operation)
    return shared.run(operation, host, iterations, tolerance, 'ChamferEdge', 'distance', 'Chamfer')
