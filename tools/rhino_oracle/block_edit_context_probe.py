# -*- coding: utf-8 -*-
"""Locate and select owned nested BlockEdit rows through public UI interfaces."""
import json
import os
import time


def navigate(label, payload, skip=False):
    if skip:
        import Rhino,System
        path=os.path.join(os.path.dirname(os.path.abspath(__file__)),'block-edit-context.json')
        with open(path+'.tmp','w') as stream:json.dump(dict(payload,point=[0,0],skip=True),stream)
        os.rename(path+'.tmp',path)
        deadline=time.time()+15
        while not os.path.isfile(path+'.ack') and time.time()<deadline:
            Rhino.RhinoApp.Wait();System.Threading.Thread.Sleep(10)
        if not os.path.isfile(path+'.ack'):raise ValueError('active context acknowledgement missing')
        with open(path+'.ack') as stream:
            if json.load(stream)!=payload['token']:raise ValueError('foreign active context acknowledgement')
        os.remove(path+'.ack');os.remove(path);return
    import ctypes, clr, System, Rhino
    from ctypes import wintypes
    for assembly in ('PresentationCore','PresentationFramework','WindowsBase'):
        clr.AddReference(assembly)
    from System.Windows.Interop import HwndSource
    from System.Windows.Media import VisualTreeHelper
    from System.Windows.Controls import TextBlock
    from System.Windows.Controls.Primitives import ToggleButton
    from System.Windows import Point
    user=ctypes.windll.user32
    callback=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    user.EnumWindows.argtypes=[callback,wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    handles=[]
    def visit(window,unused):
        pid=wintypes.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(pid))
        text=ctypes.create_unicode_buffer(128);user.GetWindowTextW(window,text,128)
        if pid.value==ctypes.windll.kernel32.GetCurrentProcessId() and text.value=='Block Edit':handles.append(window)
        return True
    user.EnumWindows(callback(visit),0)
    if len(handles)!=1:raise ValueError('expected one owned nested edit dialog')
    root=HwndSource.FromHwnd(System.IntPtr(handles[0])).RootVisual
    for pass_number in range(64):
        pending=[root];expanded=False;visits=0
        while pending:
            node=pending.pop();visits+=1
            if visits>10000:raise ValueError('nested dialog expansion exceeded bound')
            if isinstance(node,ToggleButton) and node.IsVisible and str(node.GetType().Name)=='TreeToggleButton' and node.IsChecked==False:
                node.IsChecked=True
                if node.Command is not None and node.Command.CanExecute(node.CommandParameter):node.Command.Execute(node.CommandParameter)
                expanded=True
            pending.extend(VisualTreeHelper.GetChild(node,i) for i in range(VisualTreeHelper.GetChildrenCount(node)))
        root.UpdateLayout()
        for unused in range(5):Rhino.RhinoApp.Wait();System.Threading.Thread.Sleep(10)
        if not expanded:break
    stack=[root];rows=[];texts=[];count=0
    while stack:
        node=stack.pop();count+=1
        if count>10000:raise ValueError('nested dialog exceeded visual tree bound')
        if isinstance(node,TextBlock) and node.IsVisible and str(node.Text)==label:
            point=node.PointToScreen(Point(node.ActualWidth/2,node.ActualHeight/2));rows.append((float(point.Y),float(point.X)))
        if isinstance(node,TextBlock) and node.IsVisible:texts.append(str(node.Text))
        stack.extend(VisualTreeHelper.GetChild(node,i) for i in range(VisualTreeHelper.GetChildrenCount(node)))
    if not rows:raise ValueError('nested definition row is not visible: '+label+'; visible labels: '+repr(texts))
    y,x=min(rows)
    path=os.path.join(os.path.dirname(os.path.abspath(__file__)),'block-edit-context.json')
    value=dict(payload,point=[x,y],skip=False)
    with open(path+'.tmp','w') as stream:json.dump(value,stream)
    os.rename(path+'.tmp',path)
    deadline=time.time()+15
    ack=path+'.ack'
    while not os.path.isfile(ack) and time.time()<deadline:
        Rhino.RhinoApp.Wait();System.Threading.Thread.Sleep(10)
    if not os.path.isfile(ack):raise ValueError('nested context click was not acknowledged')
    with open(ack) as stream:
        if json.load(stream)!=payload['token']:raise ValueError('foreign nested context acknowledgement')
    # Let the selection callback finish reconstructing the temporary scene.
    for unused in range(10):Rhino.RhinoApp.Wait();System.Threading.Thread.Sleep(10)
    os.remove(ack);os.remove(path)
