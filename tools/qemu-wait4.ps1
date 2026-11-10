# Run after building the current kernel, userspace and ramfs. Each invocation
# owns its VM and monitor port; it never terminates other QEMU instances.
param(
    [ValidateSet(1, 4)][int]$Cpus = 1,
    [int]$MonitorPort = 55561,
    [int]$TimeoutSeconds = 90
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$log = Join-Path $repo "build/wait4-qemu-$Cpus.log"
$errLog = Join-Path $repo "build/wait4-qemu-$Cpus.stderr.log"
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'

function Wait-Marker([string]$Marker) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        [string]$output = if (Test-Path $log) { Get-Content -Raw $log } else { '' }
        if ($output -match 'user-panic|Panicked|panicked|\[PF\]|DOUBLE FAULT') {
            throw "Guest fault; inspect $log"
        }
        if ($output -and $output.Contains($Marker)) { return }
        if ($vm.HasExited) { throw "QEMU exited before $Marker" }
        Start-Sleep -Milliseconds 200
    }
    throw "Timed out waiting for $Marker; inspect $log"
}

function Send-Command([string]$Line) {
    foreach ($character in $Line.ToCharArray()) {
        $key = switch ($character) {
            '_' { 'shift-minus' }
            '-' { 'minus' }
            ' ' { 'spc' }
            '/' { 'slash' }
            '.' { 'dot' }
            default { [string]$character }
        }
        $bytes = [Text.Encoding]::ASCII.GetBytes("sendkey $key 30`n")
        $monitor.Write($bytes, 0, $bytes.Length)
        Start-Sleep -Milliseconds 80
    }
    $bytes = [Text.Encoding]::ASCII.GetBytes("sendkey ret 30`n")
    $monitor.Write($bytes, 0, $bytes.Length)
}

$arguments = '-display none -no-reboot -no-shutdown ' +
    '-drive format=raw,file=build/image/mos-uefi.img,snapshot=on ' +
    '-drive if=pflash,format=raw,unit=0,file=target/ovmf/x64/code.fd,readonly=on ' +
    '-drive if=pflash,format=raw,unit=1,file=target/ovmf/x64/vars.fd,snapshot=on ' +
    "-serial file:build/wait4-qemu-$Cpus.log -monitor tcp:127.0.0.1:$MonitorPort,server,nowait " +
    "-cpu qemu64,+rdrand -m 4G -smp $Cpus -device virtio-gpu-pci " +
    '-device loader,file=build/ramfsout/ramfs.bin,addr=0x50000000'
# Do not allow a previous run's PASS marker to satisfy this run before QEMU
# opens and truncates its serial log.
if (Test-Path -LiteralPath $log) { Remove-Item -LiteralPath $log }
$vm = Start-Process -FilePath $qemu -ArgumentList $arguments -WorkingDirectory $repo `
    -WindowStyle Hidden -PassThru -RedirectStandardError $errLog
$client = $null
try {
    Wait-Marker '[shell] type a command'
    $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $MonitorPort)
    $monitor = $client.GetStream()
    Send-Command 'test_wait4'
    Wait-Marker '[wait4] PASS'
    Send-Command 'test_process_baseline'
    Wait-Marker '[process-baseline] PASS'
    Send-Command 'test_signal_state'
    Wait-Marker '[signal-exec] PASS'
    Wait-Marker '[signal-state] PASS'
    Send-Command 'test_signal_delivery'
    Wait-Marker '[signal-delivery] PASS'
    Send-Command 'test_signal_cross'
    Wait-Marker '[signal-cross] PASS'
    Send-Command 'test_signal_restart'
    Wait-Marker '[signal-restart] PASS'
    Send-Command 'test_fork_tls'
    Wait-Marker '[fork-tls] PASS'
    Send-Command 'test_sigpipe'
    Wait-Marker '[sigpipe] PASS'
    Send-Command 'test_write_boundary'
    Wait-Marker '[write-boundary] PASS'
    Send-Command 'test_read_boundary'
    Wait-Marker '[read-boundary] PASS'
    Send-Command 'test_pipe_interrupt'
    Wait-Marker '[pipe-interrupt] PASS'
    Send-Command 'test_sleep_interrupt'
    Wait-Marker '[sleep-interrupt] PASS'
    Send-Command 'test_sigchld'
    Wait-Marker '[sigchld] PASS'
    Send-Command 'test_tty_interrupt'
    Wait-Marker '[tty-interrupt] restart-key'
    Send-Command 'q'
    Wait-Marker '[tty-interrupt] buffered-key'
    Send-Command 'z'
    Wait-Marker '[tty-interrupt] PASS'
    Send-Command 'test_job_identity'
    Wait-Marker '[job-identity] PASS'
    Send-Command 'test_group_signal'
    Wait-Marker '[group-signal] PASS'
    Send-Command 'test_tty_job'
    Wait-Marker '[tty-job] PASS'
    Send-Command 'busybox sh -c /test_exit'
    Wait-Marker '----- test_exit -----'
    Send-Command 'busybox echo wait4_busybox_ok'
    # Echoed input lacks the CR/LF preceding the standalone output marker.
    Wait-Marker "`nwait4_busybox_ok"
    Write-Output "PASS: wait4, process baseline, signal state/exec/delivery/cross/restart, SIGPIPE and BusyBox pipeline/sh/echo ($Cpus CPUs). Log: $log"
} finally {
    if ($client) { $client.Dispose() }
    if (-not $vm.HasExited) { Stop-Process -Id $vm.Id }
}
