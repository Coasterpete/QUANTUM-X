// Bounded concurrent pipe draining for Windows PowerShell 5.1 / PowerShell 7.
// No application compilation, shell interpolation, console or desktop input.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading.Tasks;

namespace QuantumDesktopVerification
{
    public sealed class ProcessResult
    {
        public bool Started;
        public bool TimedOut;
        public bool CaptureComplete;
        public bool Terminated;
        public int? ExitCode;
        public string Error;
        public string Stdout = "";
        public string Stderr = "";
        public long StdoutBytes;
        public long StderrBytes;
        public double ElapsedSeconds;
    }

    public static class ProcessRunner
    {
        private sealed class Capture
        {
            private readonly object gate = new object();
            private readonly MemoryStream kept = new MemoryStream();
            private readonly int limit;
            private long total;
            public Capture(int limit) { this.limit = limit; }
            public async Task Drain(Stream source)
            {
                byte[] buffer = new byte[4096];
                int count;
                while ((count = await source.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false)) > 0)
                {
                    lock (gate)
                    {
                        total += count;
                        int retain = Math.Min(count, limit - (int)kept.Length);
                        if (retain > 0) kept.Write(buffer, 0, retain);
                    }
                }
            }
            public string Text { get { lock (gate) return Encoding.UTF8.GetString(kept.ToArray()); } }
            public long Total { get { lock (gate) return total; } }
        }

        // CommandLineToArgvW / MSVC escaping: preserve whitespace, quotes and
        // backslashes, including a trailing slash in a quoted directory.
        public static string Quote(string value)
        {
            StringBuilder quoted = new StringBuilder("\"");
            int slashes = 0;
            foreach (char c in value)
            {
                if (c == '\\') { slashes++; continue; }
                if (c == '"') quoted.Append('\\', slashes * 2 + 1);
                else quoted.Append('\\', slashes);
                quoted.Append(c);
                slashes = 0;
            }
            quoted.Append('\\', slashes * 2);
            return quoted.Append('"').ToString();
        }

        private static void KillTree(Process process)
        {
            if (process.HasExited) return;
            // .NET Framework lacks Kill(entireProcessTree). Use the Windows
            // utility only for the PID returned by our own successful Start.
            ProcessStartInfo info = new ProcessStartInfo(
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "taskkill.exe"),
                "/PID " + process.Id + " /T /F");
            info.UseShellExecute = false;
            info.CreateNoWindow = true;
            info.WindowStyle = ProcessWindowStyle.Hidden;
            using (Process killer = Process.Start(info))
            {
                if (!killer.WaitForExit(2000)) killer.Kill();
            }
            if (!process.HasExited) process.Kill();
        }

        public static ProcessResult Run(string executable, string[] arguments,
            string workingDirectory, IDictionary<string, string> environment,
            int timeoutMilliseconds, int limitBytes)
        {
            ProcessResult result = new ProcessResult();
            Capture stdout = new Capture(limitBytes), stderr = new Capture(limitBytes);
            Stopwatch watch = Stopwatch.StartNew();
            using (Process process = new Process())
            {
                try
                {
                    List<string> escaped = new List<string>();
                    foreach (string arg in arguments) escaped.Add(Quote(arg));
                    ProcessStartInfo info = new ProcessStartInfo(executable, string.Join(" ", escaped));
                    info.WorkingDirectory = workingDirectory;
                    info.UseShellExecute = false;
                    info.CreateNoWindow = true;
                    info.WindowStyle = ProcessWindowStyle.Hidden;
                    info.RedirectStandardOutput = true;
                    info.RedirectStandardError = true;
                    foreach (KeyValuePair<string, string> pair in environment)
                    {
                        if (string.IsNullOrEmpty(pair.Value)) info.EnvironmentVariables.Remove(pair.Key);
                        else info.EnvironmentVariables[pair.Key] = pair.Value;
                    }
                    process.StartInfo = info;
                    result.Started = process.Start();
                    Task outTask = stdout.Drain(process.StandardOutput.BaseStream);
                    Task errTask = stderr.Drain(process.StandardError.BaseStream);
                    if (!process.WaitForExit(timeoutMilliseconds))
                    {
                        result.TimedOut = true;
                        KillTree(process);
                    }
                    result.Terminated = process.WaitForExit(2000);
                    if (result.Terminated) result.ExitCode = process.ExitCode;
                    // A descendant holding an inherited pipe cannot hang us.
                    result.CaptureComplete = Task.WaitAll(new[] { outTask, errTask }, 2000);
                }
                catch (Exception error)
                {
                    // Do not leak executable paths or arbitrary exception text.
                    result.Error = error.GetType().Name;
                }
                finally
                {
                    if (result.Started)
                    {
                        try
                        {
                            if (!process.HasExited) KillTree(process);
                            result.Terminated = process.WaitForExit(2000);
                            if (result.Terminated) result.ExitCode = process.ExitCode;
                        }
                        catch { result.Terminated = false; }
                    }
                    result.Stdout = stdout.Text;
                    result.Stderr = stderr.Text;
                    result.StdoutBytes = stdout.Total;
                    result.StderrBytes = stderr.Total;
                    result.ElapsedSeconds = Math.Round(watch.Elapsed.TotalSeconds, 3);
                }
            }
            return result;
        }
    }
}
