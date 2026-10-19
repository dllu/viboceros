"""Bounded input to the public Weight dialog in the owned Rhino process."""


def watch(host, inputs, dialog, pick=False):
    import ctypes, clr, System
    from ctypes import wintypes as W
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    user = ctypes.windll.user32
    callback = ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    user.EnumWindows.argtypes = [callback, W.LPARAM]
    user.EnumChildWindows.argtypes = [W.HWND, callback, W.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [W.HWND, ctypes.POINTER(W.DWORD)]
    user.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, ctypes.c_int]
    user.GetClassNameW.argtypes = [W.HWND, W.LPWSTR, ctypes.c_int]
    user.SetWindowTextW.argtypes = [W.HWND, W.LPCWSTR]
    user.SendMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
    user.SendMessageW.restype = W.LPARAM
    pid = ctypes.windll.kernel32.GetCurrentProcessId()
    state = dict(applied=False, dialogs=[], prompts=[], errors=[], consumed=[], default=None, last_send=None, selection_opened=False)
    started = System.DateTime.UtcNow
    normalize = lambda s: s.replace('&', '').replace(' ', '').lower()

    def owned(window):
        process = W.DWORD()
        user.GetWindowThreadProcessId(window, ctypes.byref(process))
        return process.value == pid and user.IsWindowVisible(window)

    def info(window):
        text = ctypes.create_unicode_buffer(1024)
        kind = ctypes.create_unicode_buffer(128)
        user.GetWindowTextW(window, text, 1024)
        user.GetClassNameW(window, kind, 128)
        return dict(handle=int(window), text=text.value, kind=kind.value)

    def children(window):
        rows = []
        def visit(w, unused):
            if owned(w): rows.append(info(w))
            return True
        user.EnumChildWindows(window, callback(visit), 0)
        return rows

    def tick(sender, args):
        if state.get('busy'): return
        state['busy'] = True
        try:
            windows = []
            def visit(w, unused):
                if owned(w): windows.append(info(w))
                return True
            user.EnumWindows(callback(visit), 0)
            dialogs = [w for w in windows if normalize(w['text']) in ('setcontrolpointweight', 'weight')]
            prompt = str(host['Rhino'].RhinoApp.CommandPrompt)
            if not state['prompts'] or state['prompts'][-1] != prompt: state['prompts'].append(prompt)
            if dialogs and not state['applied']:
                window = dialogs[0]['handle']; rows = children(window)
                state['dialogs'].append(dict(title=dialogs[0]['text'], controls=rows))
                edits = [r for r in rows if 'edit' in r['kind'].lower()]
                if len(edits) != 3: raise ValueError('expected owned Weight field and slider range fields')
                try: state['default'] = float(edits[0]['text'])
                except ValueError: state['default'] = None
                numbers = [v for v in inputs if type(v) in (int, float)]
                label = 'cancel' if inputs[-1:] == ['Cancel'] else 'ok'
                buttons = [r for r in rows if r['kind'] == 'Button' and normalize(r['text']) == label]
                if numbers:
                    if not user.SetWindowTextW(edits[0]['handle'], repr(float(numbers[-1]))): raise ValueError('Weight text rejected')
                if len(buttons) != 1: raise ValueError('owned Weight completion button missing: ' + label)
                state['applied'] = True
                state['consumed'] = list(inputs)
                user.SendMessageW(buttons[0]['handle'], 0xf5, 0, 0)
            elif not dialogs and not state['selection_opened'] and prompt.lower().startswith('select control points'):
                state['selection_opened'] = True
                host['Rhino'].RhinoApp.SendKeystrokes('_SelPt _Enter' if pick else '_Cancel', True)
            elif not dialogs and not dialog and prompt.lower().startswith('new control point weight'):
                import re
                match = re.search(r'<([^>]+)>', prompt)
                if match and state['default'] is None: state['default'] = float(match.group(1))
                elapsed = (System.DateTime.UtcNow - started).TotalMilliseconds
                if state['last_send'] is None or elapsed - state['last_send'] > 200.:
                    index = len(state['consumed'])
                    if index < len(inputs):
                        value = inputs[index]
                        state['consumed'].append(value)
                        state['last_send'] = elapsed
                        state['applied'] = True
                        host['Rhino'].RhinoApp.SendKeystrokes('_' + value if isinstance(value, str) else repr(float(value)), True)
            if (System.DateTime.UtcNow - started).TotalSeconds > 15. and not state.get('cancelled'):
                state['cancelled'] = True
                state['errors'].append('Weight input exceeded bound')
                host['Rhino'].RhinoApp.SendKeystrokes('_Cancel', True)
                for window in dialogs:
                    for row in children(window['handle']):
                        if row['kind'] == 'Button' and normalize(row['text']) == 'cancel':
                            user.SendMessageW(row['handle'], 0xf5, 0, 0)
        except Exception as e:
            state['errors'].append(str(e))
            host['Rhino'].RhinoApp.SendKeystrokes('_Cancel', True)
        finally:
            state['busy'] = False

    timer = DispatcherTimer()
    timer.Interval = System.TimeSpan.FromMilliseconds(75)
    timer.Tick += tick
    timer.Start()
    return timer, state
