"""Prescribed command-file pauses, nested history controls and echo."""
HOLDOUTS={'undo_inside': {'text': '_Point w1,2,3 _Point w4,5,6 _Undo _Point w7,8,9\n'},
 'redo_inside': {'text': '_Point w1,2,3 _Undo _Redo _Point w4,5,6\n'},
 'paused_line': {'text': '! _Line _Pause w4,3,2\n', 'actions': [[0, 0, 0]]},
 'multi_pause_polyline': {'text': '! _Polyline _MultiPause _Enter _Point w9,9,9\n',
                          'actions': [[0, 0, 0], [4, 0, 0], [4, 3, 1], 'Enter']},
 'incomplete_line': {'text': '! _Line w0,0,0\n', 'actions': ['Cancel']},
 'no_echo': {'text': '_NoEcho ! _Point w1,2,3 _Point w4,5,6 _Echo\n'}}
