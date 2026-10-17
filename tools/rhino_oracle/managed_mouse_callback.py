"""Create an owned MouseCallback subclass whose handler stays entirely managed."""
def create(handler,host):
    # Emit only this small adapter, using the documented SDK virtual method.
    # No instructions from Rhino assemblies are read or copied.
    import clr
    S,R=host['System'],host['Rhino']
    from System.Reflection import BindingFlags,MethodAttributes,FieldAttributes,AssemblyName
    from System.Reflection.Emit import AssemblyBuilder,AssemblyBuilderAccess,OpCodes
    base=clr.GetClrType(R.UI.MouseCallback)
    override=base.GetMethod('OnEndMouseMove',BindingFlags.Instance|BindingFlags.NonPublic)
    argument=override.GetParameters()[0].ParameterType
    delegate=clr.GetClrType(S.Action[S.Object]).GetGenericTypeDefinition().MakeGenericType(S.Array[S.Type]([argument]))
    name='OwnedOracleMouse'+str(S.Guid.NewGuid()).replace('-','')
    assembly=AssemblyBuilder.DefineDynamicAssembly(AssemblyName(name),AssemblyBuilderAccess.RunAndCollect)
    module=assembly.DefineDynamicModule(name)
    from System.Reflection import TypeAttributes
    builder=module.DefineType(name,TypeAttributes.Public|TypeAttributes.Sealed,base)
    builder.DefineDefaultConstructor(MethodAttributes.Public)
    field=builder.DefineField('Handler',delegate,FieldAttributes.Public|FieldAttributes.Static)
    method=builder.DefineMethod('OnEndMouseMove',MethodAttributes.Family|MethodAttributes.Virtual|MethodAttributes.HideBySig,clr.GetClrType(S.Void),S.Array[S.Type]([argument]))
    il=method.GetILGenerator();il.Emit(OpCodes.Ldsfld,field);il.Emit(OpCodes.Ldarg_1);il.Emit(OpCodes.Callvirt,delegate.GetMethod('Invoke'));il.Emit(OpCodes.Ret)
    builder.DefineMethodOverride(method,override)
    kind=builder.CreateType();kind.GetField('Handler').SetValue(None,handler(argument,delegate))
    return S.Activator.CreateInstance(kind)
