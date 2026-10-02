using System.Collections.Concurrent;

namespace OracleCombat;

/// <summary>
/// Single-threaded SynchronizationContext that mimics Godot's main-thread continuation model: every await
/// continuation (and every Task.Delay wake-up) is queued here and executed in FIFO order on the thread that
/// calls Run*. This makes the game's async code (turn loop, action executor, hooks) deterministic.
/// </summary>
public sealed class Pump : SynchronizationContext
{
    private readonly BlockingCollection<(SendOrPostCallback cb, object state)> _q = new();
    private readonly int _threadId = Environment.CurrentManagedThreadId;

    public static Pump Instance { get; private set; }

    public static Pump Install()
    {
        var p = new Pump();
        Instance = p;
        SetSynchronizationContext(p);
        return p;
    }

    public int Pending => _q.Count;

    public override void Post(SendOrPostCallback d, object state) => _q.Add((d, state));

    public override void Send(SendOrPostCallback d, object state)
    {
        if (Environment.CurrentManagedThreadId == _threadId) { d(state); return; }
        using var done = new ManualResetEventSlim();
        Exception err = null;
        _q.Add((s => { try { d(s); } catch (Exception e) { err = e; } finally { done.Set(); } }, state));
        done.Wait();
        if (err != null) throw err;
    }

    public override SynchronizationContext CreateCopy() => this;

    /// <summary>Run all queued continuations (including ones queued while running) until the queue is empty.</summary>
    public int Drain()
    {
        int n = 0;
        while (_q.TryTake(out var item))
        {
            item.cb(item.state);
            n++;
        }
        return n;
    }

    /// <summary>
    /// Pump until <paramref name="done"/> is true and the queue is empty. If the queue is empty but
    /// <paramref name="done"/> is false, wait for timer wake-ups (Task.Delay) up to <paramref name="timeoutMs"/>.
    /// </summary>
    public void RunUntil(Func<bool> done, Func<string> describeStuck, int timeoutMs = 20000)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        while (true)
        {
            Drain();
            if (Fatal.IsSet) return;
            if (done()) return;
            if (_q.TryTake(out var item, 5))
            {
                item.cb(item.state);
                sw.Restart();
                continue;
            }
            if (sw.ElapsedMilliseconds > timeoutMs)
                throw new OracleException("stuck: no progress for " + timeoutMs + "ms. " + describeStuck());
        }
    }
}

public sealed class OracleException : Exception
{
    public OracleException(string msg) : base(msg) { }
}

/// <summary>Collects fatal problems (game Log.Error, exceptions in fire-and-forget tasks).</summary>
public static class Fatal
{
    public static string Message;
    public static bool IsSet => Message != null;
    public static bool Lenient;
    public static readonly List<string> Warnings = new();
    public static void Set(string m) { if (Message == null) Message = m; }
}
