"""Fixed layer, nested-file and failure observations."""
EXTRAS={'layer_table': {'text': '! _-Layer _New "Macro layer" _Enter _Point w1,2,3\n', 'layers': True},
 'layer_options': {'text': '! _-Layer _New "Macro layer" _Off "Macro layer" _On "Macro layer" _Lock '
                           '"Macro layer" _Unlock "Macro layer" _Current "Macro layer" _Enter _Point '
                           'w1,2,3\n',
                   'layers': True},
 'nested_file': {'text': '_Point w1,2,3 _-ReadCommandFile "@CHILD@" _Point w7,8,9\n',
                 'child': '_Point w4,5,6\n'},
 'nested_bang': {'text': '_Point w1,2,3 _-ReadCommandFile "@CHILD@" _Point w7,8,9\n',
                 'child': '! _Point w4,5,6\n'},
 'unknown_command': {'text': '_Point w1,2,3 _VibocerosMissingCommand _Point w4,5,6\n'},
 'last_selection': {'text': '_Point w1,2,3 _Point w4,5,6\n', 'last_selection': True}}
