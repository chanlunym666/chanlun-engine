# Build + Deploy slzs_chanlun.dll to TDX (64-bit DLL)
# 构建脚本: 在本仓库 (chanlun_kaiyuan) 上构建并部署到本机通达信
# TDX 仅支持 64 位 DLL → 使用默认 x86_64-pc-windows-mscv
# ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
# Usage: powershell -File build_deploy.ps1
#
# 特性:
#   1. 自动探测通达信 T0002\dlls 目录 (多盘符/常用安装位置扫描, 也可手动填下方 $TdxDllDir)
#   2. 部署后自动绑定 DLL3 (写 T0002\dlls\dlls.ini 的 [BAND] band3=slzs_chanlun.dll;
#      无 ini 自动重建标准 10 槽; band3 被占先备份再替换; 幂等可重复运行)
#   3. 目标目录可写探测 → 不可写自动请求 UAC 提权重跑 (弹一次"是"即完成, 无需手动管理员)
#   4. 部署前检测通达信是否运行 (运行中 DLL 被锁, 复制会失败)
#   5. 部署后校验: 时间戳 + SHA256 + ini band3 回读

param([switch]$Force, [string]$TdxDllDir = "")

$ErrorActionPreference = "Stop"

$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path   # 开源副本 (chanlun_kaiyuan/tdx_dll)
$DllName = "slzs_chanlun.dll"

# ── 路径配置: 留空则自动探测 (注册表/进程/常用位置); 探测失败时在此手动填, 例: D:\通达信\T0002\dlls ──
# (UAC 提权重跑时该目录经 -TdxDllDir 参数透传, 无需重复探测)

function Find-TdxDllDir {
    # 1) 注册表卸载信息里的通达信安装目录 (最准, 覆盖自定义安装盘)
    $regHits = @()
    foreach ($root in @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
                        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
                        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*')) {
        $regHits += Get-ItemProperty $root -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -match '通达信' -and $_.InstallLocation } |
            ForEach-Object { $_.InstallLocation.TrimEnd('\') }
    }
    $candidates = @()
    foreach ($r in $regHits) {
        $candidates += (Join-Path $r "T0002\dlls")
    }
    # 2) 已运行的通达信进程路径
    $proc = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^(TdxW|TdxSP|TdxNonSP|AutoUpEx|Tdxw)$' } | Select-Object -First 1
    if ($proc -and $proc.Path) {
        $root = Split-Path -Parent (Split-Path -Parent $proc.Path)   # ...\TdxW.exe → 安装根
        $candidates += (Join-Path $root "T0002\dlls")
    }
    # 3) 常用安装位置兜底
    foreach ($drive in @('C:','D:','E:','F:')) {
        foreach ($sub in @(
            "${drive}\new_tdx\T0002\dlls",
            "${drive}\通达信\T0002\dlls",
            "${drive}\tdx\T0002\dlls",
            "${drive}\T0002\dlls"
        )) { $candidates += $sub }
    }
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) { return $c }
    }
    return ""
}

if (-not $TdxDllDir -or -not (Test-Path $TdxDllDir)) {
    $TdxDllDir = Find-TdxDllDir
    if ($TdxDllDir) {
        Write-Host "[AUTO] 探测到通达信 dlls 目录: $TdxDllDir" -ForegroundColor Cyan
    } else {
        Write-Host "ERROR: 未能自动探测到通达信 T0002\dlls 目录. 请在脚本开头配置 `$TdxDllDir." -ForegroundColor Red
        exit 1
    }
}

function Test-DirWritable {
    param([string]$Dir)
    try {
        $probe = Join-Path $Dir (".write_probe_" + [guid]::NewGuid().ToString("N") + ".tmp")
        [System.IO.File]::WriteAllText($probe, "x")
        Remove-Item $probe -Force -ErrorAction SilentlyContinue
        return $true
    } catch {
        return $false
    }
}

function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Set-TdxDllBinding {
    param([string]$IniPath, [string]$DllName)
    if (-not (Test-Path $IniPath)) {
        # 无 ini → 整体重建标准 10 槽
        $lines = @("[BAND]") + (1..10 | ForEach-Object { "band$_=" })
        $lines[3] = "band3=$DllName"   # [BAND]=0, band1=1, band2=2, band3=3
        [System.IO.File]::WriteAllLines($IniPath, $lines, [System.Text.Encoding]::ASCII)
        return "created"
    }
    $content = [System.IO.File]::ReadAllLines($IniPath)
    for ($i = 0; $i -lt $content.Length; $i++) {
        if ($content[$i] -match '^band3=') {
            $old = ($content[$i] -split '=', 2)[1]
            if ($old -eq $DllName) {
                return "already"      # 幂等: 已绑定, 不动
            }
            $bak = $IniPath + ".bak_" + (Get-Date -Format "MMdd")
            if (-not (Test-Path $bak)) { Copy-Item $IniPath $bak -Force }
            $content[$i] = "band3=$DllName"
            [System.IO.File]::WriteAllLines($IniPath, $content, [System.Text.Encoding]::ASCII)
            return "replaced(原band3=$old, 旧ini备份:$bak)"
        }
    }
    # 有 ini 无 band3 行 → 追加 band3
    $content += "band3=$DllName"
    [System.IO.File]::WriteAllLines($IniPath, $content, [System.Text.Encoding]::ASCII)
    return "appended"
}

Write-Host "=== Chanlun DLL: Build + Deploy + Bind ===" -ForegroundColor Cyan

# ── Step 0: 通达信进程检测 (运行中 DLL 被锁, 复制必然失败) ──
$tdxProcs = Get-Process -ErrorAction SilentlyContinue | Where-Object {
    $_.ProcessName -match '^(TdxW|TdxSP|TdxNonSP|AutoUpEx|Tdxw)$'
}
if ($tdxProcs) {
    if ($Force) {
        Write-Host "[0/6] WARN(-Force): 通达信正在运行, 复制可能失败: $($tdxProcs.ProcessName -join ', ')" -ForegroundColor Yellow
    } else {
        Write-Host "[0/6] 通达信正在运行, 请先完全退出再部署! 进程: $($tdxProcs.ProcessName -join ', ')" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "[0/6] 通达信进程检查: 未运行, 可部署" -ForegroundColor Green
}

# ── Step 0.5: 目标目录可写探测 → 不可写自动 UAC 提权重跑 ──
if (-not (Test-DirWritable -Dir $TdxDllDir)) {
    if (Test-IsAdmin) {
        Write-Host "[0.5/6] 目录仍不可写(ACL 特殊), 请手动处理: $TdxDllDir" -ForegroundColor Red
        exit 1
    }
    Write-Host "[0.5/6] 目标目录需要管理员权限, 正在请求提升(UAC 请点\"是\")..." -ForegroundColor Yellow
    $uacArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-TdxDllDir', "`"$TdxDllDir`"")
    if ($Force) { $uacArgs += '-Force' }
    Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $uacArgs -Wait
    if (Test-DirWritable -Dir $TdxDllDir) {
        Write-Host "[OK] 提权实例部署完成" -ForegroundColor Green
        exit 0
    } else {
        Write-Host "[FAIL] 提权被拒绝或部署未完成" -ForegroundColor Red
        exit 1
    }
}
Write-Host "[0.5/6] 目标目录可写: 无需管理员" -ForegroundColor Green

# ── Step 1: Build (64-bit, default x86_64-pc-windows-mscv) ──
Write-Host "[1/6] cargo build --release (64-bit) ..." -ForegroundColor Yellow
Push-Location $ProjectDir
# PS5.1 兼容: cargo 把进度写 stderr, 2>&1 合并时会被 Stop 策略当错误中断 → 该段临时降级
$eapBak = $ErrorActionPreference; $ErrorActionPreference = "Continue"
& cargo build --release 2>&1 | ForEach-Object { Write-Host $_ }
$buildExit = $LASTEXITCODE
$ErrorActionPreference = $eapBak
if ($buildExit -ne 0) {
    Write-Host "BUILD FAILED! (exit $buildExit)" -ForegroundColor Red
    Pop-Location
    exit 1
}
Write-Host "[OK] Build succeeded" -ForegroundColor Green

# ── Step 2: Test ──
Write-Host "[2/6] cargo test ..." -ForegroundColor Yellow
$eapBak = $ErrorActionPreference; $ErrorActionPreference = "Continue"
& cargo test 2>&1 | ForEach-Object { Write-Host $_ }
$testExit = $LASTEXITCODE
$ErrorActionPreference = $eapBak
if ($testExit -ne 0) {
    Write-Host "TEST FAILED! (exit $testExit)" -ForegroundColor Red
    Pop-Location
    exit 1
}
Write-Host "[OK] Tests passed" -ForegroundColor Green
Pop-Location

# ── Step 3: Copy to TDX ──
$src = Join-Path $ProjectDir "target\release\$DllName"
$dst = Join-Path $TdxDllDir $DllName

if (-not (Test-Path $src)) {
    Write-Host "DLL NOT FOUND: $src" -ForegroundColor Red
    exit 1
}

Write-Host "[3/6] Deploy to: $dst" -ForegroundColor Yellow
Copy-Item -Path $src -Destination $dst -Force
Write-Host "[OK] Deployed" -ForegroundColor Green

# ── Step 4: 自动绑定 DLL3 (写 dlls.ini [BAND] band3) ──
$ini = Join-Path $TdxDllDir "dlls.ini"
$bindResult = Set-TdxDllBinding -IniPath $ini -DllName $DllName
Write-Host "[4/6] DLL3 绑定: $bindResult" -ForegroundColor Green

# ── Step 5: Verify (时间戳 + SHA256 + ini 回读) ──
Write-Host "[5/6] Verify..." -ForegroundColor Yellow
$srcTime = (Get-Item $src).LastWriteTime
$dstTime = (Get-Item $dst).LastWriteTime
if ($srcTime -ne $dstTime) {
    Write-Host "TIMESTAMP MISMATCH! src=$srcTime dst=$dstTime" -ForegroundColor Red
    exit 1
}
$sha = [System.Security.Cryptography.SHA256]::Create()
$hash = [BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($dst))).Replace('-', '')
$readBack = [System.IO.File]::ReadAllLines($ini) | Where-Object { $_ -match '^band3=' }
if ($readBack -notmatch [regex]::Escape($DllName)) {
    Write-Host "BIND VERIFY FAILED! ini band3 回读异常: $readBack" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Version consistent ($($dstTime.ToString('yyyy-MM-dd HH:mm:ss')))" -ForegroundColor Green
Write-Host "[OK] SHA256: $hash" -ForegroundColor Green
Write-Host "[OK] band3 回读: $readBack" -ForegroundColor Green

Write-Host ""
Write-Host "=== Deploy Complete (Build+Deploy+Bind) ===" -ForegroundColor Cyan
Write-Host "DLL:  $dst" -ForegroundColor White
Write-Host "Time: $($dstTime.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor White
Write-Host "重启通达信后生效 (DLL3 已自动绑定, 无需手工绑定)" -ForegroundColor Yellow
