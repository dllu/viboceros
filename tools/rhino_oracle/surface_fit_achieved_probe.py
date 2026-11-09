"""Public RhinoFitSurface achieved-tolerance output through documented interop."""
import math

SYMBOL=b'?RhinoFitSurface@@YAPEAVON_Surface@@AEBV1@HHNPEAN@Z'


def validate(op):
    if globals().get('__package__'):from . import surface_fit_probe
    else:import surface_fit_probe
    if not isinstance(op,dict)or op.get('op')!='surface_fit_achieved':
        raise ValueError('invalid achieved surface fit operation')
    adapted=dict(op);adapted['op']='surface_fit';surface_fit_probe.validate(adapted)


def public_function():
    import ctypes
    kernel=ctypes.windll.kernel32
    kernel.GetModuleHandleW.argtypes=[ctypes.c_wchar_p];kernel.GetModuleHandleW.restype=ctypes.c_void_p
    kernel.GetProcAddress.argtypes=[ctypes.c_void_p,ctypes.c_char_p];kernel.GetProcAddress.restype=ctypes.c_void_p
    for name in ['RhinoCore.dll','Rhino.exe']:
        module=kernel.GetModuleHandleW(name)
        if not module:continue
        address=kernel.GetProcAddress(module,SYMBOL)
        if address:
            # Win64 has one calling convention for this public SDK signature.
            signature=ctypes.WINFUNCTYPE(ctypes.c_void_p,ctypes.c_void_p,ctypes.c_int,ctypes.c_int,
                                        ctypes.c_double,ctypes.POINTER(ctypes.c_double))
            return signature(address),name
    raise ValueError('documented RhinoFitSurface export not found')


def run(op,host,iterations,tolerance):
    import ctypes
    validate(op);R=host['Rhino'];S=host['System']
    function,module=public_function()
    source=host['_nurbs_surface_from_definition'](op['surface']);define=host['_nurbs_surface_definition']
    try:
        before=define(source)
        def compute():
            achieved=ctypes.c_double(float('nan'))
            managed=source.Fit(op['degree_u'],op['degree_v'],op['fit_tolerance'])
            cpp=managed_nurbs=cpp_nurbs=None
            try:
                pointer=R.Runtime.Interop.NativeGeometryConstPointer(source).ToInt64()
                result=function(pointer,op['degree_u'],op['degree_v'],op['fit_tolerance'],ctypes.byref(achieved))
                if result:cpp=R.Runtime.Interop.CreateFromNativePointer(S.IntPtr(result))
                if managed is not None:managed_nurbs=managed.ToNurbsSurface()
                if cpp is not None:cpp_nurbs=cpp.ToNurbsSurface()
                after=define(source)
                if before!=after:raise ValueError('public fit changed its source')
                finite=not math.isnan(achieved.value)and not math.isinf(achieved.value)
                return dict(before=before,after=after,module=module,
                    achieved_tolerance=achieved.value if finite else None,achieved_finite=finite,
                    managed_surface=define(managed_nurbs)if managed_nurbs is not None else None,
                    surface=define(cpp_nurbs)if cpp_nurbs is not None else None)
            finally:
                if cpp_nurbs is not None and cpp_nurbs is not cpp:cpp_nurbs.Dispose()
                if managed_nurbs is not None and managed_nurbs is not managed:managed_nurbs.Dispose()
                if cpp is not None:cpp.Dispose()
                if managed is not None:managed.Dispose()
        return host['_measure'](iterations,compute)
    finally:source.Dispose()
