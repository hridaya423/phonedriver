$env:DO_NOT_TRACK = "1"
$env:PHONE_HARNESS_TELEMETRY = "0"
$env:PHONE_HARNESS_NO_UPDATE_CHECK = "1"
$env:PHONE_HARNESS_PLATFORM = "android"
$env:ANDROID_SERIAL = "emulator-5554"
$p = Start-Process -PassThru -NoNewWindow -FilePath "$PSScriptRoot\ph\Scripts\python.exe" -RedirectStandardOutput "$PSScriptRoot\probe.out" -ArgumentList "$PSScriptRoot\outbound_probe.py"
Write-Output "pid $($p.Id)"
$seen = @{}
while (-not $p.HasExited) {
    $ids = @($p.Id) + @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)" | ForEach-Object { $_.ProcessId })
    foreach ($id in $ids) {
        Get-NetTCPConnection -OwningProcess $id -ErrorAction SilentlyContinue | ForEach-Object {
            $key = "pid-role=$(if ($id -eq $p.Id) {'python'} else {'child'}) $($_.LocalAddress):$($_.LocalPort) -> $($_.RemoteAddress):$($_.RemotePort) $($_.State)"
            $seen[($key -replace ':\d+ ->', ':* ->')] = $key
        }
    }
    Start-Sleep -Milliseconds 25
}
Write-Output "distinct connections seen from the python process and its children:"
$seen.Values | Sort-Object | ForEach-Object { Write-Output $_ }
$nonLocal = $seen.Values | Where-Object { $_ -notmatch '-> 127\.0\.0\.1:' -and $_ -notmatch '-> ::1:' }
if ($nonLocal) { Write-Output "NON-LOOPBACK: $nonLocal" } else { Write-Output "no connection to any address other than loopback" }

Write-Output "--- python audit hook output ---"
Get-Content "$PSScriptRoot\probe.out"
