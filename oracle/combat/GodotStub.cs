using System.Reflection;
using System.Runtime.InteropServices;

namespace OracleCombat;

public static unsafe class GodotStub
{
    [DllImport("libc", EntryPoint = "mmap")]
    private static extern IntPtr mmap(IntPtr addr, nuint length, int prot, int flags, int fd, nint offset);
    [DllImport("kernel32")]
    private static extern IntPtr VirtualAlloc(IntPtr addr, nuint size, uint type, uint protect);

    public static void Install()
    {
        IntPtr page = OperatingSystem.IsWindows()
            ? VirtualAlloc(IntPtr.Zero, 4096, 0x3000 , 0x40 )
            : mmap(IntPtr.Zero, 4096, 7 , 0x22 , -1, 0);
        if (page == new IntPtr(-1) || page == IntPtr.Zero) throw new Exception("mmap failed");
        byte* p = (byte*)page;
        p[0] = 0x48; p[1] = 0xB8; *(long*)(p + 2) = (long)page; p[10] = 0xC3;

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
