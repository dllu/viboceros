"""Enumerate and click exact labels in a panel owned by the current Rhino PID."""


def inspect(title, action=None, physical=False):
    import ctypes
    from ctypes import wintypes
    user = ctypes.windll.user32
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback, wintypes.LPARAM]
    user.EnumChildWindows.argtypes = [wintypes.HWND, callback, wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.IsWindowEnabled.argtypes = [wintypes.HWND]
    user.SendMessageW.argtypes = [wintypes.HWND, ctypes.c_uint, wintypes.WPARAM, wintypes.LPARAM]
    rows, buttons = [], []

    def label(window):
        text, kind = ctypes.create_unicode_buffer(512), ctypes.create_unicode_buffer(128)
        user.GetWindowTextW(window, text, 512)
        user.GetClassNameW(window, kind, 128)
        return text.value, kind.value

    def child(window, unused):
        text, kind = label(window)
        rows.append(dict(label=text, kind=kind, enabled=bool(user.IsWindowEnabled(window)),
                         check=int(user.SendMessageW(window, 0x00F0, 0, 0)) if kind == 'Button' else None))
        if kind == 'Button':
            buttons.append((' '.join(text.replace('&', '').split()), window))
        return True

    def visit(window, unused):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(window, ctypes.byref(pid))
        text, kind = label(window)
        if (pid.value == ctypes.windll.kernel32.GetCurrentProcessId()
                and user.IsWindowVisible(window) and title in text):
            rows.append(dict(label=text, kind=kind, check=None))
            user.EnumChildWindows(window, callback(child), 0)
        return True

    user.EnumWindows(callback(visit), 0)
    if action is not None:
        matches = [window for text, window in buttons if text.lower() == action.lower()]
        if len(matches) != 1:
            raise ValueError('owned panel control unavailable: ' + action + '; controls=' + str(rows))
        if not user.IsWindowEnabled(matches[0]):
            raise ValueError('owned panel control is disabled: '+action)
        user.GetAncestor.argtypes=[wintypes.HWND,ctypes.c_uint]
        user.GetAncestor.restype=wintypes.HWND
        user.SetActiveWindow.argtypes=[wintypes.HWND]
        user.SetActiveWindow(user.GetAncestor(matches[0],2))
        user.SetFocus.argtypes=[wintypes.HWND]
        user.SetFocus(matches[0])
        if physical:
            user.SetForegroundWindow.argtypes=[wintypes.HWND]
            user.SetForegroundWindow(user.GetAncestor(matches[0],2))
            rect=wintypes.RECT();point=wintypes.POINT()
            user.GetClientRect.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.RECT)]
            user.ClientToScreen.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.POINT)]
            if not user.GetClientRect(matches[0],ctypes.byref(rect)):raise ValueError('owned control rectangle missing')
            point.x=(rect.right-rect.left)//2;point.y=(rect.bottom-rect.top)//2
            if not user.ClientToScreen(matches[0],ctypes.byref(point)):raise ValueError('owned control screen point missing')
            if not (0<=point.x<user.GetSystemMetrics(0) and 0<=point.y<user.GetSystemMetrics(1)):raise ValueError('owned control outside private screen')
            user.SetCursorPos(point.x,point.y)
            user.mouse_event.argtypes=[wintypes.DWORD,wintypes.DWORD,wintypes.DWORD,wintypes.DWORD,ctypes.c_size_t]
            user.mouse_event(2,0,0,0,0);user.mouse_event(4,0,0,0,0)
        else:
            user.SendMessageW(matches[0], 0x00F5, 0, 0)
    return rows
