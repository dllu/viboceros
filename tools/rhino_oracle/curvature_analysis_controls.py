"""Bounded native panel actions, restricted to the current owned Rhino PID."""
def click_max_range():
    import ctypes
    from ctypes import wintypes
    user=ctypes.windll.user32
    callback=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    user.EnumWindows.argtypes=[callback,wintypes.LPARAM]
    user.EnumChildWindows.argtypes=[wintypes.HWND,callback,wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.GetClassNameW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.IsWindowVisible.argtypes=[wintypes.HWND]
    user.SendMessageW.argtypes=[wintypes.HWND,wintypes.UINT,wintypes.WPARAM,wintypes.LPARAM]
    matches=[];labels=[]
    def child(window,unused):
        label=ctypes.create_unicode_buffer(512);kind=ctypes.create_unicode_buffer(128)
        user.GetWindowTextW(window,label,512);user.GetClassNameW(window,kind,128)
        labels.append(label.value)
        if kind.value=='Button' and label.value.replace('&','').replace(' ','').lower()=='maxrange':matches.append(window)
        return True
    def visit(window,unused):
        pid=wintypes.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(pid))
        label=ctypes.create_unicode_buffer(512);user.GetWindowTextW(window,label,512)
        if pid.value==ctypes.windll.kernel32.GetCurrentProcessId() and user.IsWindowVisible(window) and 'Curvature' in label.value:
            user.EnumChildWindows(window,callback(child),0)
        return True
    user.EnumWindows(callback(visit),0)
    if len(matches)!=1:raise ValueError('owned Max Range control unavailable: '+str(labels))
    user.SendMessageW(matches[0],0x00F5,0,0)
