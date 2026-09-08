# Build + Deploy slzs_chanlun_mt4.dll to MT4 (32-bit DLL)
# 构建脚本: 在本仓库 (chanlun_kaiyuan) 上构建并部署到本机 MT4
# MT4 仅支持 32 位 DLL → 必须 --target i686-pc-windows-msvc
# ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
# Usage: powershell -File build_deploy.ps1
#
# 5 步流程:
#   1. cargo build --release --target i686-pc-windows-msvc
#   2. cargo test
#   3. Copy DLL to MT4 Libraries\
#   4. Copy MQL4 wrapper to MT4 Indicators\
#   5. Timestamp verification

$ErrorActionPreference = "Continue"

$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path   # 开源副本 (chanlun_kaiyuan/mt4_dll)
$DllName = "slzs_chanlun_mt4.dll"
$Mql4Name = "chanlun.mq4"
$Target = "i686-pc-windows-msvc"   # MT4 是 32 位, DLL 必须编译为 32 位

# ── 路径配置: 按你的 MT4 环境填写; 留空则自动探测 ──
$Mt4Libraries = ""    # 你的 MT4 数据目录 MQL4\Libraries, 例: C:\Users\<你>\AppData\Roaming\MetaQuotes\Terminal\<ID>\MQL4\Libraries
$Mt4Indicators = ""   # 你的 MT4 数据目录 MQL4\Indicators (chanlun.mq4 放置处)
$MetaEditor = ""      # metaeditor.exe 完整路径, 例: D:\MT4\metaeditor.exe

# 自动探测 MT4 数据目录 (AppData\Roaming\MetaQuotes\Terminal\<ID>\MQL4, 取最近使用)
if (-not $Mt4Libraries -or -not (Test-Path $Mt4Libraries)) {
    $mql4Base = Get-ChildItem "$env:APPDATA\MetaQuotes\Terminal" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "MQL4" } |
        Where-Object { Test-Path $_ } |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($mql4Base) {
        $Mt4Libraries = Join-Path $mql4Base "Libraries"
        $Mt4Indicators = Join-Path $mql4Base "Indicators"
        Write-Host "[AUTO] MT4 data dir: $mql4Base" -ForegroundColor Cyan
    }
}
if (-not (Test-Path $Mt4Libraries)) {
    Write-Host "ERROR: MT4 Libraries 目录未找到. 请在脚本开头配置 `$Mt4Libraries." -ForegroundColor Red
    exit 1
}

# 自动探测 metaeditor.exe (WebInstall 目录)
if (-not $MetaEditor -or -not (Test-Path $MetaEditor)) {
    $me = Get-ChildItem "$env:APPDATA\MetaQuotes\WebInstall" -Recurse -Filter "metaeditor.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($me) { $MetaEditor = $me.FullName }
}
if (-not (Test-Path $MetaEditor)) {
    Write-Host "ERROR: metaeditor.exe 未找到. 请在脚本开头配置 `$MetaEditor." -ForegroundColor Red
    exit 1
}

Write-Host "=== ChanLun MT4 DLL: Build + Deploy ===" -ForegroundColor Cyan
Write-Host ""

# ── Step 0: Check MT4 not running ──
$mt4Running = Get-Process -Name "terminal" -ErrorAction SilentlyContinue
if ($mt4Running) {
    Write-Host "[WARN] MT4 is running! Close MT4 before deploying DLL." -ForegroundColor Yellow
    Write-Host "       DLL copy will fail if MT4 holds file lock." -ForegroundColor Yellow
}

# ── Step 1: Build (⚠️ 32-bit, MT4 仅支持 i686) ──
Write-Host "[1/5] cargo build --release --target $Target (32-bit) ..." -ForegroundColor Yellow
Push-Location $ProjectDir
$buildResult = cargo build --release --target $Target 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED!" -ForegroundColor Red
    Write-Host $buildResult
    Pop-Location
    exit 1
}
Write-Host "[OK] Build succeeded" -ForegroundColor Green

# ── Step 2: Test ──
Write-Host "[2/5] cargo test ..." -ForegroundColor Yellow
$testResult = cargo test 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "TEST FAILED!" -ForegroundColor Red
    Write-Host $testResult
    Pop-Location
    exit 1
}
Write-Host "[OK] Tests passed" -ForegroundColor Green
Pop-Location

# ── Step 3: Copy to MT4 Libraries ──
$src = Join-Path $ProjectDir "target\$Target\release\$DllName"
$dst = Join-Path $Mt4Libraries $DllName

if (-not (Test-Path $src)) {
    Write-Host "DLL NOT FOUND: $src" -ForegroundColor Red
    exit 1
}

Write-Host "[3/5] Deploy DLL to: $dst" -ForegroundColor Yellow
Copy-Item -Path $src -Destination $dst -Force
Write-Host "[OK] DLL deployed" -ForegroundColor Green

# ── Step 4: MQL4 唯一源在 MT4 Indicators (mql4/ 目录不再存放副本) ──
$mql4Dst = Join-Path $Mt4Indicators $Mql4Name
$ex4Dst = [System.IO.Path]::ChangeExtension($mql4Dst, ".ex4")
# metaeditor 已在前置配置区探测, 此处直接使用

Write-Host "[4/5] MQL4 source: $mql4Dst (唯一副本)" -ForegroundColor Yellow
if (Test-Path $mql4Dst) {
    # 删除旧 .ex4 后用 metaeditor 主动重编译
    # ⚠️ MT4 启动/加载指标时不会自动编译缺失的 ex4,
    #    仅删 ex4 会导致指标加载失败 — 必须主动编译生成新 ex4
    if (Test-Path $ex4Dst) {
        Remove-Item $ex4Dst -Force
        Write-Host "[OK] Old .ex4 removed" -ForegroundColor Green
    }
    $compileLog = Join-Path $Mt4Indicators "compile_deploy.log"
    Remove-Item $compileLog -Force -ErrorAction SilentlyContinue
    & $MetaEditor /compile:"$mql4Dst" /log:"$compileLog" | Out-Null
    Start-Sleep -Seconds 3
    if (Test-Path $ex4Dst) {
        Write-Host "[OK] .ex4 recompiled: $((Get-Item $ex4Dst).Length) bytes" -ForegroundColor Green
    } else {
        Write-Host "[FAIL] .ex4 compile failed! Check log: $compileLog" -ForegroundColor Red
        if (Test-Path $compileLog) { Get-Content $compileLog -Encoding Unicode }
        Pop-Location
        exit 1
    }
    Write-Host "[OK] MQL4 source confirmed: $mql4Dst" -ForegroundColor Green
} else {
    Write-Host "MQL4 NOT FOUND: $mql4Dst" -ForegroundColor Yellow
}

# ── Step 5: Verify consistency ──
Write-Host "[5/5] Verify DLL consistency..." -ForegroundColor Yellow
$srcTime = (Get-Item $src).LastWriteTime
$dstTime = (Get-Item $dst).LastWriteTime

if ($srcTime -eq $dstTime) {
    Write-Host "[OK] Version consistent ($($srcTime.ToString('HH:mm:ss')))" -ForegroundColor Green
} else {
    Write-Host "TIMESTAMP MISMATCH! src=$srcTime dst=$dstTime" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== Deploy Complete ===" -ForegroundColor Cyan
Write-Host "DLL:  $dst" -ForegroundColor White
Write-Host "Time: $($srcTime.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor White
Write-Host ""
Write-Host "WARNING: Close MT4 before overwriting DLL!" -ForegroundColor Yellow
