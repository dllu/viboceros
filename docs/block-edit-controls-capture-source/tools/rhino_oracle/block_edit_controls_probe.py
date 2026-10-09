# -*- coding: utf-8 -*-
"""Drive owned BlockEdit controls through public WPF/Win32 UI interfaces."""
import json
import os
import time


def invoke(label, payload):
    import clr
    import ctypes
    import System
    from ctypes import wintypes
    for assembly in ('PresentationCore', 'PresentationFramework', 'WindowsBase'):
        clr.AddReference(assembly)
    from System.Windows.Interop import HwndSource
    from System.Windows.Media import VisualTreeHelper
    from System.Windows.Controls import Button
    from System.Windows import Point
    user = ctypes.windll.user32
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback, wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    handles = []
    def visit(window, unused):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(window, ctypes.byref(pid))
        text = ctypes.create_unicode_buffer(128)
        user.GetWindowTextW(window, text, 128)
        if pid.value == ctypes.windll.kernel32.GetCurrentProcessId() and text.value == 'Block Edit':
            handles.append(window)
        return True
    user.EnumWindows(callback(visit), 0)
    if len(handles) != 1:
        raise ValueError('expected one owned Block Edit dialog')
    root = HwndSource.FromHwnd(System.IntPtr(handles[0])).RootVisual
    stack = [root]
    buttons = []
    visits = 0
    while stack:
        node = stack.pop()
        visits += 1
        if visits > 10000:
            raise ValueError('Block Edit visual tree exceeded its bound')
        if isinstance(node, Button) and node.IsVisible and str(node.Content).endswith(': ' + label):
            buttons.append(node)
        stack.extend(VisualTreeHelper.GetChild(node, i) for i in range(VisualTreeHelper.GetChildrenCount(node)))
    if len(buttons) != 1 or not buttons[0].IsEnabled:
        raise ValueError('owned Block Edit button unavailable: ' + label)
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'block-edit-control.json')
    import Rhino
    ready = path + '.ready'
    initial_prompt = str(Rhino.RhinoApp.CommandPrompt)
    completed = path + '.completed'
    def prompt_changed(sender, args):
        prompt = str(Rhino.RhinoApp.CommandPrompt)
        if prompt and prompt != initial_prompt and not os.path.isfile(ready):
            with open(ready + '.tmp', 'w') as stream:
                json.dump(dict(token=payload['token'], prompt=prompt), stream)
            os.rename(ready + '.tmp', ready)
        if os.path.isfile(ready) and buttons[0].IsEnabled and not os.path.isfile(completed):
            with open(completed + '.tmp', 'w') as stream:
                json.dump(payload['token'], stream)
            os.rename(completed + '.tmp', completed)
    from System.Windows.Threading import DispatcherTimer
    timer = DispatcherTimer()
    timer.Interval = System.TimeSpan.FromMilliseconds(50)
    timer.Tick += prompt_changed
    timer.Start()
    point=buttons[0].PointToScreen(Point(buttons[0].ActualWidth/2,buttons[0].ActualHeight/2))
    payload=dict(payload,button=[float(point.X),float(point.Y)])
    with open(path + '.tmp', 'w') as stream:
        json.dump(payload, stream)
    os.rename(path + '.tmp', path)
    # The host acknowledgement is required even if an empty getter returned.
    ack = path + '.ack'
    deadline = time.time() + 15
    try:
        while not os.path.isfile(ack) and time.time() < deadline:
            Rhino.RhinoApp.Wait()
            System.Threading.Thread.Sleep(10)
    finally:
        timer.Stop()
        timer.Tick -= prompt_changed
    if not os.path.isfile(ack):
        raise ValueError('Block Edit control input was not acknowledged')
    with open(ack) as stream:
        if json.load(stream) != payload['token']:
            raise ValueError('foreign Block Edit control acknowledgement')
    os.remove(ack)
    os.remove(path)
    if os.path.isfile(ready):
        os.remove(ready)
    if os.path.isfile(completed):
        os.remove(completed)
