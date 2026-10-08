using System.Collections.Concurrent;

namespace AgentBridge;

public static class MainThread
{
    private static readonly ConcurrentQueue<Action> _queue = new();
    private static readonly List<(Func<bool> done, TaskCompletionSource tcs, long deadline)> _waits = new();

    public static long Frame { get; private set; }

    internal static void Pump()
    {
        Frame++;
        while (_queue.TryDequeue(out var a))
        {
            try { a(); }
            catch (Exception e) { Main.Log.Info($"AgentBridge main-thread error: {e}", 0); }
        }
        for (int i = _waits.Count - 1; i >= 0; i--)
        {
            var (done, tcs, deadline) = _waits[i];
            bool ok;
            try { ok = done(); } catch { ok = false; }
            if (ok || Environment.TickCount64 > deadline)
            {
                _waits.RemoveAt(i);
                tcs.TrySetResult();
            }
        }
    }

    public static Task<T> Run<T>(Func<T> f)
    {
        var tcs = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        _queue.Enqueue(() =>
        {
            try { tcs.SetResult(f()); }
            catch (Exception e) { tcs.SetException(e); }
        });
        return tcs.Task;
    }

    public static Task Until(Func<bool> done, int timeoutMs)
    {
        var tcs = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        _waits.Add((done, tcs, Environment.TickCount64 + timeoutMs));
        return tcs.Task;
    }
}
