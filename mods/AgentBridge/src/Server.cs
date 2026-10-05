using System.Net;
using System.Net.Sockets;
using System.Text;

namespace AgentBridge;

/// <summary>
/// One request per connection: the client sends one line (a command), the server answers with plain text and closes.
/// Commands: `s` (state), `a <args>` (act, then wait until the game settles and return the new state), `d` (deck),
/// `p <pile>` (draw|discard|exhaust), `m` (full map), `x <cmd>` (dev console command).
/// </summary>
public static class Server
{
    public static void Start(int port)
    {
        var listener = new TcpListener(IPAddress.Loopback, port);
        listener.Start();
        var th = new Thread(() =>
        {
            while (true)
            {
                TcpClient c;
                try { c = listener.AcceptTcpClient(); }
                catch { continue; }
                _ = Task.Run(() => Handle(c));
            }
        }) { IsBackground = true, Name = "AgentBridge" };
        th.Start();
    }

    private static readonly SemaphoreSlim _one = new(1, 1);

    private static async Task Handle(TcpClient c)
    {
        using (c)
        {
            var stream = c.GetStream();
            string reply;
            try
            {
                using var reader = new StreamReader(stream, Encoding.UTF8, false, 4096, leaveOpen: true);
                string line = (await reader.ReadLineAsync())?.Trim() ?? "";
                await _one.WaitAsync();
                try { reply = await Commands.Execute(line); }
                finally { _one.Release(); }
            }
            catch (Exception e)
            {
                reply = "ERR " + e.GetBaseException().Message;
            }
            var bytes = Encoding.UTF8.GetBytes(reply.EndsWith('\n') ? reply : reply + "\n");
            try { await stream.WriteAsync(bytes); } catch { }
        }
    }
}
