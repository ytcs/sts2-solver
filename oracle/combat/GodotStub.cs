using System.Reflection;
using System.Runtime.InteropServices;

namespace OracleCombat;

/// <summary>
/// Makes every Godot native function pointer (GodotSharp NativeFuncs._unmanagedCallbacks) point to a stub
/// that returns 0 and does nothing. Lets the game's managed logic run with no Godot engine: any native call
/// becomes a harmless no-op (empty strings/arrays, null handles).
/// </summary>
public static unsafe class GodotStub
{
    [DllImport("libc", EntryPoint = "mmap")]
    private static extern IntPtr mmap(IntPtr addr, nuint length, int prot, int flags, int fd, nint offset);

    public static void Install()
    {
        // x86-64: mov rax, <own address> ; ret  (non-null so method-bind lookups succeed)
        IntPtr page = mmap(IntPtr.Zero, 4096, 7 /*RWX*/, 0x22 /*PRIVATE|ANON*/, -1, 0);
        if (page == new IntPtr(-1)) throw new Exception("mmap failed");
        byte* p = (byte*)page;
        p[0] = 0x48; p[1] = 0xB8; *(long*)(p + 2) = (long)page; p[10] = 0xC3;  // mov rax, imm64(self); ret

        var asm = typeof(Godot.GD).Assembly;
        var nf = asm.GetType("Godot.NativeInterop.NativeFuncs");
        var ucType = nf.GetNestedType("UnmanagedCallbacks", BindingFlags.NonPublic | BindingFlags.Public);
        int n = ucType.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic).Length;
        int size = n * sizeof(IntPtr);
        IntPtr buf = Marshal.AllocHGlobal(size);
        for (int i = 0; i < n; i++) ((IntPtr*)buf)[i] = page;
        nf.GetMethod("Initialize", BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic).Invoke(null, new object[] { buf, size });
    }
}
