while ($true) {
  $procs = Get-CimInstance Win32_Process -Filter "Name='python.exe'" | Where-Object { $_.CommandLine -like '*baalor*' }
  if (-not $procs) { break }
  foreach ($c in $procs) {
    $p = Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue
    if ($p) {
      $free = (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1e6
      Add-Content -Path "$PSScriptRoot\..\out\watchdog.log" -Value ("{0} pid={1} ws={2:N2} commit={3:N2} free={4:N1}" -f (Get-Date -Format HH:mm:ss), $c.ProcessId, ($p.WorkingSet64/1e9), ($p.PrivateMemorySize64/1e9), $free)
      if ($p.WorkingSet64 -gt 6e9 -or $p.PrivateMemorySize64 -gt 12e9 -or $free -lt 6) { Stop-Process -Id $c.ProcessId -Force; Add-Content -Path "$PSScriptRoot\..\out\watchdog.log" -Value "KILLED $($c.ProcessId)" }
    }
  }
  Start-Sleep -Seconds 5
}
