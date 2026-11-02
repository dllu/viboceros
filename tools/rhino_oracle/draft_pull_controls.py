"""Public WPF controls, discovered only below the current owned Rhino panel."""
def panel(move_for_pick=False):
    import ctypes
    import clr
    from ctypes import wintypes as W
    clr.AddReference('PresentationCore')
    clr.AddReference('PresentationFramework')
    from System import IntPtr
    from System.Windows.Interop import HwndSource
    from System.Windows.Media import VisualTreeHelper as V
    from System.Windows.Controls import ComboBox, Button, TextBlock
    U=ctypes.windll.user32
    callback=ctypes.WINFUNCTYPE(W.BOOL,W.HWND,W.LPARAM)
    U.EnumWindows.argtypes=[callback,W.LPARAM]
    U.EnumChildWindows.argtypes=[W.HWND,callback,W.LPARAM]
    U.GetWindowThreadProcessId.argtypes=[W.HWND,ctypes.POINTER(W.DWORD)]
    U.GetWindowTextW.argtypes=[W.HWND,W.LPWSTR,ctypes.c_int]
    U.GetClassNameW.argtypes=[W.HWND,W.LPWSTR,ctypes.c_int]
    U.IsWindowVisible.argtypes=[W.HWND]
    roots=[];owners=[]
    def child(window,unused):
        kind=ctypes.create_unicode_buffer(128);U.GetClassNameW(window,kind,128)
        if kind.value.startswith('HwndWrapper'):
            source=HwndSource.FromHwnd(IntPtr(int(window)))
            if source is not None and source.RootVisual is not None:roots.append(source.RootVisual)
        return True
    def top(window,unused):
        pid=W.DWORD();U.GetWindowThreadProcessId(window,ctypes.byref(pid))
        title=ctypes.create_unicode_buffer(512);U.GetWindowTextW(window,title,512)
        if pid.value==ctypes.windll.kernel32.GetCurrentProcessId() and U.IsWindowVisible(window) and 'Draft' in title.value:
            owners.append(window);U.EnumChildWindows(window,callback(child),0)
        return True
    U.EnumWindows(callback(top),0)
    if len(owners)!=1 or len(roots)!=1:raise ValueError('owned WPF draft panel unavailable')
    if move_for_pick:
        U.SetWindowPos.argtypes=[W.HWND,W.HWND,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.UINT]
        if not U.SetWindowPos(owners[0],None,10,180,0,0,0x15):raise ValueError('could not move owned panel clear of viewport')
    def walk(node):
        yield node
        for i in range(V.GetChildrenCount(node)):
            for item in walk(V.GetChild(node,i)):yield item
    nodes=list(walk(roots[0]))
    combos=[n for n in nodes if isinstance(n,ComboBox)]
    def label(node):return ' '.join(str(n.Text) for n in walk(node) if isinstance(n,TextBlock))
    buttons=[n for n in nodes if isinstance(n,Button) and label(n)=='Set...']
    if len(combos)!=1 or len(buttons)!=1:raise ValueError('owned direction selector or Set button missing')
    combo=combos[0]
    choices=[str(x.Content) if hasattr(x,'Content') else str(x) for x in combo.Items]
    return combo,buttons[0],choices


def choose(label):
    combo,button,choices=panel()
    if choices.count(label)!=1:raise ValueError('owned direction choice missing: '+label)
    combo.SelectedIndex=choices.index(label)
    return button,choices


def click(button):
    from System.Windows import RoutedEventArgs
    from System.Windows.Controls.Primitives import ButtonBase
    button.RaiseEvent(RoutedEventArgs(ButtonBase.ClickEvent))


def prepare(label):
    combo,button,choices=panel()
    if label not in choices:raise ValueError('owned direction choice missing: '+label)
    if combo.SelectedIndex==choices.index(label):
        alternate='World Z' if label!='World Z' else 'CPlane Z'
        combo.SelectedIndex=choices.index(alternate)
