"""Public SDK BlendEdge recipes; licensed capture remains pending."""
import json
from pathlib import Path
try:
    from . import edge_blend_probe as shared
except ImportError:
    import edge_blend_probe as shared

def validate(operation):
    shared.validate(operation, 'blend_edge', 'radius', 'BlendEdge')

def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('blend_edge.json').read_text())

def run(operation, host, iterations, tolerance):
    validate(operation)
    return shared.run(operation, host, iterations, tolerance, 'BlendEdge', 'radius', 'Blend')
