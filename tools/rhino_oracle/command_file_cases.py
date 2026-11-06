"""Literal command-file bytes; no input is derived from native geometry."""
from copy import deepcopy
CASES={'line': {'text': '! _Line w0,0,0 w4,3,2\n'},
 'two_points': {'text': '_Point w1,2,3 _Point w4,5,6\n'},
 'polyline': {'text': '! _Polyline\nw0,0,0\nw4,0,0\nw4,3,1\n_Enter\n'},
 'grid': {'text': '! _SrfControlPtGrid _Degree=1 2 _Degree=1 3\n'
                  'w0,0,0 w0,2,1 w0,5,0\n'
                  'w4,0,0 w4,2,3 w4,5,2\n'},
 'comments': {'text': '; comment-only line\n_Point w1,2,3 ; ignored _Point w90,90,90\n_Point w4,5,6\n'},
 'bang_middle': {'text': '_Point w1,2,3 ! _Point w4,5,6\n'},
 'bang_incomplete': {'text': '! _Line w0,0,0 ! _Point w4,5,6\n'},
 'explicit_enter': {'text': '_Point w1,2,3 _Enter\n_Point w4,5,6 _Enter\n'},
 'empty_lines': {'text': '\n\n_Point w1,2,3\n\n_Point w4,5,6\n\n'},
 'quoted_layer': {'text': '! _-Layer _New "Macro layer" _Enter _Point w1,2,3\n'},
 'utf8_bom': {'text': '_Point w1,2,3\n', 'encoding': 'utf-8-sig'},
 'utf16_le': {'text': '_Point w4,5,6\n', 'encoding': 'utf-16'}}

PILOT_NAMES=list(CASES)
if __package__:from .command_file_holdouts import HOLDOUTS
else:from command_file_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .command_file_extras import EXTRAS
else:from command_file_extras import EXTRAS
CASES.update(EXTRAS)
if __package__:from .command_file_polyline import POLYLINES
else:from command_file_polyline import POLYLINES
CASES.update(POLYLINES)

def request(names=None):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='command_file',id=n,case=n) for n in (PILOT_NAMES if names is None else names)])
def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='command_file' or not isinstance(op.get('case'),str) or op['case'] not in CASES or op.get('id')!=op['case']:
        raise ValueError('invalid fixed command-file recipe')
def recipe(name):return deepcopy(CASES[name])
