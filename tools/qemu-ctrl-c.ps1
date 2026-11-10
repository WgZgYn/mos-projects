# Exercise the real static BusyBox ash through MOS's keyboard/tty path.
param(
    [ValidateSet(1, 4, 8, 16)][int]$Cpus = 1,
    [int]$MonitorPort = 55571,
    [int]$TimeoutSeconds = 45,
    [switch]$TerminalOnly
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$log = Join-Path $repo "build/ctrl-c-qemu-$Cpus.log"
$errLog = Join-Path $repo "build/ctrl-c-qemu-$Cpus.stderr.log"
$qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
$script:OutputStart = 0

function Guest-Output {
    if (Test-Path -LiteralPath $log) { return Get-Content -Raw -LiteralPath $log }
    return ''
}

function Wait-Marker([string]$Marker) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        [string]$output = Guest-Output
        if ($output -match 'user-panic|Panicked|panicked|\[PF\]|DOUBLE FAULT') {
            throw "Guest fault; inspect $log"
        }
        if ($output -and $output.Length -ge $script:OutputStart) {
            $fresh = $output.Substring($script:OutputStart) -replace '\x1b\[[0-?]*[ -/]*[@-~]', ''
            if ($fresh.Contains($Marker)) { return }
        }
        if ($vm.HasExited) { throw "QEMU exited before $Marker" }
        Start-Sleep -Milliseconds 150
    }
    throw "Timed out waiting for $Marker; inspect $log"
}

function Send-Monitor([string]$Command) {
    $bytes = [Text.Encoding]::ASCII.GetBytes($Command + "`n")
    $monitor.Write($bytes, 0, $bytes.Length)
    $monitor.Flush()
}

function Send-Command([string]$Line) {
    foreach ($character in $Line.ToCharArray()) {
        $key = switch ($character) {
            '_' { 'shift-minus' }
            '-' { 'minus' }
            ' ' { 'spc' }
            '/' { 'slash' }
            '|' { 'shift-backslash' }
            '&' { 'shift-7' }
            '%' { 'shift-5' }
            '.' { 'dot' }
            '+' { 'shift-equal' }
            default { [string]$character }
        }
        Send-Monitor "sendkey $key 20"
        Start-Sleep -Milliseconds 45
    }
    Send-Monitor 'sendkey ret 20'
}

function Send-ControlC {
    Send-Monitor 'sendkey ctrl-c 40'
}

function Assert-Output([string]$Command, [string]$Marker) {
    $script:OutputStart = (Guest-Output).Length
    Send-Command $Command
    Wait-Marker $Marker
    $script:OutputStart = 0
}

function Assert-Interrupt([string]$Command, [string]$Started, [string]$Marker) {
    Send-Command $Command
    if ($Started) { Wait-Marker $Started } else { Start-Sleep -Milliseconds 500 }
    Send-ControlC
    Start-Sleep -Milliseconds 150
    Send-Command "echo $Marker"
    Wait-Marker "`n$Marker"
}

$arguments = '-display none -no-reboot -no-shutdown ' +
    '-drive format=raw,file=build/image/mos-uefi.img,snapshot=on ' +
    '-drive if=pflash,format=raw,unit=0,file=target/ovmf/x64/code.fd,readonly=on ' +
    '-drive if=pflash,format=raw,unit=1,file=target/ovmf/x64/vars.fd,snapshot=on ' +
    "-serial file:build/ctrl-c-qemu-$Cpus.log -monitor tcp:127.0.0.1:$MonitorPort,server,nowait " +
    "-cpu qemu64,+rdrand -m 4G -smp $Cpus -device virtio-gpu-pci " +
    '-device loader,file=build/ramfsout/ramfs.bin,addr=0x50000000'
if (Test-Path -LiteralPath $log) { Remove-Item -LiteralPath $log }
$vm = Start-Process -FilePath $qemu -ArgumentList $arguments -WorkingDirectory $repo `
    -WindowStyle Hidden -PassThru -RedirectStandardError $errLog
$client = $null
try {
    Wait-Marker '[shell] type a command'
    $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $MonitorPort)
    $monitor = $client.GetStream()

    Send-Command 'test_tty_job'
    Wait-Marker '[tty-job] PASS'
    Send-Command 'test_frontend'
    Wait-Marker '[frontend] PASS'
    Send-Command 'test_terminal'
    Wait-Marker '[terminal-layer] covered'
    $covered = Join-Path $repo "build/terminal-covered-$Cpus.ppm"
    if (Test-Path -LiteralPath $covered) { Remove-Item -LiteralPath $covered }
    Send-Monitor "screendump build/terminal-covered-$Cpus.ppm"
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Test-Path -LiteralPath $covered)) {
        if ((Get-Date) -gt $deadline) { throw 'Window-layer screenshot timed out' }
        Start-Sleep -Milliseconds 100
    }
    Send-Command ''
    Wait-Marker '[terminal] PASS'
    $screenshot = Join-Path $repo "build/terminal-qemu-$Cpus.ppm"
    if (Test-Path -LiteralPath $screenshot) { Remove-Item -LiteralPath $screenshot }
    Send-Monitor "screendump build/terminal-qemu-$Cpus.ppm"
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Test-Path -LiteralPath $screenshot)) {
        if ((Get-Date) -gt $deadline) { throw 'Terminal screenshot timed out' }
        Start-Sleep -Milliseconds 100
    }
    Send-Command 'test_terminal_scroll'
    Wait-Marker '[terminal-scroll] PASS'
    $scroll = Join-Path $repo "build/terminal-scroll-$Cpus.ppm"
    if (Test-Path -LiteralPath $scroll) { Remove-Item -LiteralPath $scroll }
    Send-Monitor "screendump build/terminal-scroll-$Cpus.ppm"
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Test-Path -LiteralPath $scroll)) {
        if ((Get-Date) -gt $deadline) { throw 'Scroll screenshot timed out' }
        Start-Sleep -Milliseconds 100
    }
    if ($TerminalOnly) {
        Write-Output "PASS: terminal byte-stream probe ($Cpus CPUs). Screenshot: $screenshot"
        return
    }
    Send-Command 'test_shell_probe'
    Wait-Marker '[shell-probe] PASS'
    Send-Command 'busybox sh'
    Start-Sleep -Milliseconds 500

    # Require actual fresh command output, not echoed input or an old prompt.
    Assert-Output 'echo shell_echo_ok' "`nshell_echo_ok"
    Assert-Output 'cd /mem && pwd' "`n/mem"
    Assert-Output 'cd /dev && /busybox pwd' "`n/dev"
    Assert-Output 'cd && pwd' "`n/"
    Assert-Output 'busybox ls /busybox' "`n/busybox"
    Assert-Output 'busybox date +date_ok' "`ndate_ok"
    Assert-Output 'busybox ls /busybox && echo ls_status_ok' "`nls_status_ok"

    Assert-Interrupt '/test_spin' '[ctrl-c-spin] running' 'ctrlc_spin_ok'
    Assert-Interrupt '/busybox sleep 30' '' 'ctrlc_sleep_ok'
    Assert-Interrupt '/busybox cat' '' 'ctrlc_cat_ok'
    Assert-Interrupt '/busybox sleep 30 | /busybox cat' '' 'ctrlc_pipe_ok'

    # A separate background pgrp must not receive the foreground SIGINT.
    Send-Command '/busybox sleep 3 &'
    Start-Sleep -Milliseconds 150
    Assert-Interrupt '/busybox sleep 30' '' 'ctrlc_foreground_ok'
    Wait-Marker 'Done'
    Send-Command 'echo ctrlc_background_ok'
    Wait-Marker "`nctrlc_background_ok"

    # Ctrl-C at ash's own prompt must redraw/continue instead of exiting ash.
    Send-ControlC
    Start-Sleep -Milliseconds 150
    Send-Command 'echo ctrlc_idle_ok'
    Wait-Marker "`nctrlc_idle_ok"
    Send-Command 'exit'
    Write-Output "PASS: BusyBox ash Ctrl-C (spin/sleep/cat/pipeline/idle; background isolated, $Cpus CPUs). Log: $log"
} finally {
    if ($client) { $client.Dispose() }
    if (-not $vm.HasExited) { Stop-Process -Id $vm.Id }
}
