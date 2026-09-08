# qemu-run.ps1 — build + boot MOS in QEMU, wait for a serial-log marker.
#
# Usage (run from repo root or anywhere):
#   .\tools\qemu-run.ps1                            # build all, boot, wait for shell ready
#   .\tools\qemu-run.ps1 -Wait "hello from"         # custom serial marker
#   .\tools\qemu-run.ps1 -SkipBuild                 # boot existing image without building
#   .\tools\qemu-run.ps1 -Target kernel             # kernel-only rebuild
#   .\tools\qemu-run.ps1 -KillAfterWait             # quit VM once the marker is seen
#   .\tools\qemu-run.ps1 -ShowTrace                 # dump panic/syscall trace lines
#
# Handles the usual pitfalls: stale QEMU holding the serial log / monitor port,
# slow VM shutdown, and cwd dependence of cargo (always builds from repo root).

param(
    [string]$Target = "all",            # cargo run --release -- build --target <this>
    [switch]$SkipBuild,
    [string]$Wait = "ready to get input",
    [int]$TimeoutSec = 90,
    [switch]$KillAfterWait,
    [switch]$ShowTrace
)

$ErrorActionPreference = "Stop"
$Repo    = Split-Path -Parent $PSScriptRoot
$Qemu    = "C:\Program Files\qemu\qemu-system-x86_64.exe"
$Log     = Join-Path $Repo "qemu_serial.log"
$MonPort = 55555

function Send-Monitor([string[]]$Cmds) {
    try {
        $c = New-Object System.Net.Sockets.TcpClient
        $c.Connect('127.0.0.1', $MonPort)
        $s = $c.GetStream()
        foreach ($cmd in $Cmds) {
            $b = [System.Text.Encoding]::ASCII.GetBytes($cmd + "`r`n")
            $s.Write($b, 0, $b.Length)
            Start-Sleep -Milliseconds 150
        }
        $s.Flush()
        $c.Close()
    } catch {
        Write-Host "[qemu-run] monitor not reachable: $($_.Exception.Message)"
    }
}

# 1. evict any stale VM holding the serial log / monitor port
$stale = Get-Process -Name "qemu-system-x86_64" -ErrorAction SilentlyContinue
if ($stale) {
    Write-Host "[qemu-run] stale QEMU (pid $($stale.Id)) - quitting via monitor..."
    Send-Monitor @('quit')
    Start-Sleep -Seconds 3
    $stale = Get-Process -Name "qemu-system-x86_64" -ErrorAction SilentlyContinue
    if ($stale) {
        Write-Host "[qemu-run] monitor quit didn't work, force killing"
        $stale | Stop-Process -Force
    }
}

# 2. build (cargo must run from the repo root)
if (-not $SkipBuild) {
    Push-Location $Repo
    cargo run --release -- build --target $Target
    if ($LASTEXITCODE -ne 0) { Pop-Location; throw "build failed (target=$Target)" }
    Pop-Location
}

# 3. fresh serial log + launch
Remove-Item $Log -Force -ErrorAction SilentlyContinue
$qemuArgs = '-drive "format=raw,file=build/image/mos-uefi.img" ' +
    '-drive "if=pflash,format=raw,unit=0,file=target/ovmf/x64/code.fd,readonly=on" ' +
    '-drive "if=pflash,format=raw,unit=1,file=target/ovmf/x64/vars.fd,snapshot=on" ' +
    '-serial "file:qemu_serial.log" ' +
    "-monitor `"tcp:127.0.0.1:${MonPort},server,nowait`" " +
    '-cpu "qemu64,+rdrand" -m 4G -smp 4 ' +
    '-device virtio-gpu-pci ' +
    '-audiodev "dsound,id=snd0" -machine "pcspk-audiodev=snd0" ' +
    '-device "loader,file=build/ramfsout/ramfs.bin,addr=0x50000000"'
$qemuErr = Join-Path $Repo "qemu_stderr.log"
Remove-Item $qemuErr -Force -ErrorAction SilentlyContinue
Start-Process -FilePath $Qemu -ArgumentList $qemuArgs -WorkingDirectory $Repo `
    -RedirectStandardError $qemuErr
Write-Host "[qemu-run] QEMU launched (log: $Log, stderr: $qemuErr)"

# 4. wait for the marker in the serial log
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$found = $false
while ((Get-Date) -lt $deadline) {
    if (Test-Path $Log) {
        if (Select-String -Path $Log -Pattern $Wait -Quiet) { $found = $true; break }
    }
    Start-Sleep -Milliseconds 500
}
if (-not $found) {
    Write-Host "[qemu-run] TIMEOUT (${TimeoutSec}s) waiting for '$Wait'"
    Get-Content $Log -Tail 20 -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "[qemu-run] marker found: $Wait"

# 5. show the interesting part of the log
if ($ShowTrace) {
    Select-String -Path $Log -Pattern "sys-trace|user-panic|\[PF\]|Panicked|unknown syscall|exited with" |
        Select-Object -Last 40 | ForEach-Object { $_.Line }
} else {
    Get-Content $Log -Tail 15
}

# 6. optional teardown
if ($KillAfterWait) {
    Send-Monitor @('quit')
    Write-Host "[qemu-run] VM quit"
}
