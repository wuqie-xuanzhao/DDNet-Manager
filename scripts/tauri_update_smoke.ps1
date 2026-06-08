param(
    [switch] $PrepareOnly
)

$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$fixtureRoot = Join-Path $root 'src-tauri\src\test\fixtures\update-smoke'
$tmpRoot = Join-Path $root 'tmp'
$smokeBaseRoot = Join-Path $tmpRoot 'tauri-update-smoke'
$smokeRunId = [guid]::NewGuid().ToString('N')
$smokeRoot = Join-Path $smokeBaseRoot $smokeRunId
$packageRoot = Join-Path $fixtureRoot 'package-root'
$templateManifestPath = Join-Path $fixtureRoot 'manifest.json'
$servedManifestPath = Join-Path $smokeRoot 'manifest.json'
$zipPath = Join-Path $smokeRoot 'qmclient-smoke.zip'
$clientInstallRoot = Join-Path $smokeRoot 'client-install'
$clientInstallPath = Join-Path $clientInstallRoot 'QmClient'
$stdoutLog = Join-Path $smokeRoot 'tauri-dev.out.log'
$stderrLog = Join-Path $smokeRoot 'tauri-dev.err.log'
$resultPath = Join-Path $smokeRoot 'local-smoke-result.json'
$tauriDevConfigPath = Join-Path $smokeRoot 'tauri.smoke.dev.json'
$systemTempRoot = [System.IO.Path]::GetTempPath()
$cargoTargetBaseRoot = Join-Path $systemTempRoot 'ddnet-manager-tauri-update-smoke-target'
$cargoTargetDir = Join-Path $cargoTargetBaseRoot $smokeRunId
$smokeProfileRoot = Join-Path $smokeRoot 'profile'
$smokeAppData = Join-Path $smokeProfileRoot 'AppData\Roaming'
$smokeLocalAppData = Join-Path $smokeProfileRoot 'AppData\Local'
$smokeXdgDataHome = Join-Path $smokeProfileRoot 'xdg-data'
$smokeXdgConfigHome = Join-Path $smokeProfileRoot 'xdg-config'
$smokeXdgCacheHome = Join-Path $smokeProfileRoot 'xdg-cache'
$fixturePort = 18765
$manifestUrl = "http://127.0.0.1:$fixturePort/manifest.json"
$assetUrl = "http://127.0.0.1:$fixturePort/qmclient-smoke.zip"
$listenerJob = $null
$previousSmokeEnv = $env:DDNET_MANAGER_ALLOW_LOCAL_SMOKE
$previousSmokeResultPath = $env:DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH
$previousCargoTargetDir = $env:CARGO_TARGET_DIR
$previousAppData = $env:APPDATA
$previousLocalAppData = $env:LOCALAPPDATA
$previousXdgDataHome = $env:XDG_DATA_HOME
$previousXdgConfigHome = $env:XDG_CONFIG_HOME
$previousXdgCacheHome = $env:XDG_CACHE_HOME
$previousViteLocalSmoke = $env:VITE_DDNET_MANAGER_LOCAL_SMOKE
$previousViteSmokeClientInstallDir = $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR
$previousViteSmokeManifestUrl = $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL
$previousViteSmokeCloseWindow = $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH

function Write-Step([string] $message) {
    Write-Host "[tauri-smoke-update] $message"
}

function Copy-DirectoryContent([string] $sourceDir, [string] $targetDir) {
    New-Item -ItemType Directory -Force $targetDir | Out-Null
    Copy-Item (Join-Path $sourceDir '*') -Destination $targetDir -Recurse -Force
}

function Copy-DirectoryContentExcept([string] $sourceDir, [string] $targetDir, [string[]] $excludeNames) {
    New-Item -ItemType Directory -Force $targetDir | Out-Null
    Get-ChildItem $sourceDir | Where-Object { $_.Name -notin $excludeNames } | ForEach-Object {
        Copy-Item $_.FullName -Destination (Join-Path $targetDir $_.Name) -Recurse -Force
    }
}

function Get-AvailableLoopbackPort() {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    try {
        $listener.Start()
        return ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
    }
    finally {
        $listener.Stop()
    }
}

function Assert-LoopbackPortAvailable([int] $port, [string] $label) {
    $listener = $null
    try {
        $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $port)
        $listener.Start()
    }
    catch {
        throw "$label 端口 $port 已被占用，请先释放冲突进程后再重试。"
    }
    finally {
        if ($null -ne $listener) {
            $listener.Stop()
        }
    }
}

function Read-JobDiagnostic($job) {
    if ($null -eq $job) {
        return ''
    }

    try {
        return (Receive-Job -Id $job.Id -Keep 2>&1 | Out-String).Trim()
    }
    catch {
        return ''
    }
}

function Assert-UpdatedClientBinary([string] $clientDir) {
    $binaryPath = Join-Path $clientDir 'DDNet.exe'
    if (-not (Test-Path $binaryPath)) {
        throw "updated client binary not found: $binaryPath"
    }

    $content = [System.IO.File]::ReadAllText($binaryPath)
    $normalizedContent = $content.TrimEnd([char[]]"`r`n")
    if ($normalizedContent -ne 'smoke-updated-build') {
        $bytes = [System.IO.File]::ReadAllBytes($binaryPath)
        throw "expected DDNet.exe content to be smoke-updated-build, got: $normalizedContent`nbytes=$([string]::Join(',', $bytes))"
    }
}

function Read-LocalSmokeResult([string] $path) {
    if (-not (Test-Path $path)) {
        throw "local smoke result file not found: $path"
    }

    $raw = Get-Content $path -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        throw "local smoke result file is empty: $path"
    }

    return $raw | ConvertFrom-Json
}

function Assert-LocalSmokeSucceeded([string] $path) {
    $result = Read-LocalSmokeResult $path
    if ($result.status -ne 'succeeded') {
        $message = if ([string]::IsNullOrWhiteSpace([string] $result.message)) { '<empty>' } else { [string] $result.message }
        throw "local smoke reported failure at stage $($result.stage): $message"
    }

    if ($result.stage -ne 'install') {
        throw "local smoke succeeded at unexpected stage: $($result.stage)"
    }
}

function Wait-HttpReady([string] $url, $job) {
    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        if ($null -ne $job -and $job.State -in @('Completed', 'Failed', 'Stopped')) {
            $diagnostic = Read-JobDiagnostic $job
            if ([string]::IsNullOrWhiteSpace($diagnostic)) {
                $diagnostic = "listener job state=$($job.State)"
            }
            throw "local fixture server exited before readiness check: $diagnostic"
        }

        try {
            $response = Invoke-WebRequest -Uri $url -Method Get
            if ($response.StatusCode -eq 200) {
                return
            }
        }
        catch {
        }

        Start-Sleep -Milliseconds 100
    }

    $diagnostic = Read-JobDiagnostic $job
    if ([string]::IsNullOrWhiteSpace($diagnostic)) {
        throw "local fixture server did not become ready: $url"
    }

    throw "local fixture server did not become ready: $url`n$diagnostic"
}

try {
    if (-not (Test-Path $templateManifestPath)) {
        throw "fixture manifest not found: $templateManifestPath"
    }
    if (-not (Test-Path $packageRoot)) {
        throw "fixture package root not found: $packageRoot"
    }

    Write-Step '准备本地 smoke 目录。'
    New-Item -ItemType Directory -Force $tmpRoot | Out-Null
    New-Item -ItemType Directory -Force $smokeBaseRoot | Out-Null
    New-Item -ItemType Directory -Force $smokeRoot | Out-Null

    $stagingPackageRoot = Join-Path $smokeRoot 'package-root'
    Copy-DirectoryContent $packageRoot $stagingPackageRoot

    Write-Step '生成本地当前客户端目录。'
    New-Item -ItemType Directory -Force $clientInstallRoot | Out-Null
    Copy-DirectoryContentExcept (Join-Path $packageRoot 'QmClient') $clientInstallPath @('DDNet.exe')
    Set-Content -Path (Join-Path $clientInstallPath 'DDNet.exe') -Value 'smoke-current-build' -NoNewline

    Write-Step '生成本地更新 zip 包。'
    if (Test-Path $zipPath) {
        Remove-Item $zipPath -Force -Confirm:$false
    }
    Compress-Archive -Path (Join-Path $stagingPackageRoot '*') -DestinationPath $zipPath -CompressionLevel Optimal

    $sha256 = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item $zipPath).Length

    Write-Step '回填本地 manifest。'
    $manifest = Get-Content $templateManifestPath -Raw | ConvertFrom-Json
    $manifest.clients[0].assets[0].asset_url = $assetUrl
    $manifest.clients[0].assets[0].sha256 = $sha256
    $manifest.clients[0].assets[0].size = $size
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path $servedManifestPath

    if ($PrepareOnly) {
        Write-Step 'PrepareOnly 模式：仅准备 zip 与 manifest 文件，不启动本地 HTTP 服务或 Tauri dev。'
        Write-Host ''
        Write-Host 'PrepareOnly 输出：'
        Write-Host "- smoke 根目录：$smokeRoot"
        Write-Host "- smoke 基目录：$smokeBaseRoot"
        Write-Host "- 默认客户端目录：$clientInstallPath"
        Write-Host "- 待服务 manifest 文件：$servedManifestPath"
        Write-Host "- 更新 zip：$zipPath"
        Write-Host ''
        return
    }

    Assert-LoopbackPortAvailable $fixturePort '本地 HTTP fixture'
    $vitePort = Get-AvailableLoopbackPort
    $viteDevUrl = "http://127.0.0.1:$vitePort"

    Write-Step '生成 Tauri smoke dev 配置。'
    New-Item -ItemType Directory -Force $cargoTargetBaseRoot | Out-Null
    New-Item -ItemType Directory -Force $cargoTargetDir | Out-Null
    New-Item -ItemType Directory -Force $smokeAppData | Out-Null
    New-Item -ItemType Directory -Force $smokeLocalAppData | Out-Null
    New-Item -ItemType Directory -Force $smokeXdgDataHome | Out-Null
    New-Item -ItemType Directory -Force $smokeXdgConfigHome | Out-Null
    New-Item -ItemType Directory -Force $smokeXdgCacheHome | Out-Null
    @{
        build = @{
            beforeDevCommand = "bun run dev -- --port $vitePort --strictPort"
            devUrl = $viteDevUrl
        }
    } | ConvertTo-Json -Depth 10 | Set-Content -Path $tauriDevConfigPath

    Write-Step '启动本地 HTTP fixture 服务。'
    $listenerJob = Start-Job -ArgumentList $servedManifestPath, $zipPath, $fixturePort -ScriptBlock {
        param($ManifestPath, $ZipPath, $FixturePort)

        $ErrorActionPreference = 'Stop'
        $listener = [System.Net.HttpListener]::new()
        $listener.Prefixes.Add("http://127.0.0.1:$FixturePort/")
        $listener.Start()
        try {
            while ($listener.IsListening) {
                try {
                    $context = $listener.GetContext()
                }
                catch {
                    break
                }

                $requestPath = $context.Request.Url.AbsolutePath
                switch ($requestPath) {
                    '/manifest.json' {
                        $bytes = [System.IO.File]::ReadAllBytes($ManifestPath)
                        $context.Response.StatusCode = 200
                        $context.Response.ContentType = 'application/json; charset=utf-8'
                        $context.Response.ContentLength64 = $bytes.LongLength
                        $context.Response.OutputStream.Write($bytes, 0, $bytes.Length)
                    }
                    '/qmclient-smoke.zip' {
                        $bytes = [System.IO.File]::ReadAllBytes($ZipPath)
                        $context.Response.StatusCode = 200
                        $context.Response.ContentType = 'application/zip'
                        $context.Response.ContentLength64 = $bytes.LongLength
                        $context.Response.OutputStream.Write($bytes, 0, $bytes.Length)
                    }
                    default {
                        $bytes = [System.Text.Encoding]::UTF8.GetBytes('not found')
                        $context.Response.StatusCode = 404
                        $context.Response.ContentType = 'text/plain; charset=utf-8'
                        $context.Response.ContentLength64 = $bytes.LongLength
                        $context.Response.OutputStream.Write($bytes, 0, $bytes.Length)
                    }
                }

                $context.Response.OutputStream.Close()
            }
        }
        finally {
            if ($listener.IsListening) {
                $listener.Stop()
            }
            $listener.Close()
        }
    }

    Wait-HttpReady $manifestUrl $listenerJob

    Write-Step '本地 fixture 已准备完成。'
    Write-Host ''
    Write-Host 'Smoke 使用信息：'
    Write-Host "- smoke 根目录：$smokeRoot"
    Write-Host "- smoke 基目录：$smokeBaseRoot"
    Write-Host "- 默认客户端目录：$clientInstallPath"
    Write-Host "- 本地 manifest URL：$manifestUrl"
    Write-Host "- 更新 zip：$zipPath"
    Write-Host "- 结果文件：$resultPath"
    Write-Host "- Vite dev URL：$viteDevUrl"
    Write-Host "- Tauri smoke 配置：$tauriDevConfigPath"
    Write-Host "- Tauri cargo target：$cargoTargetDir"
    Write-Host "- Tauri stdout 日志：$stdoutLog"
    Write-Host "- Tauri stderr 日志：$stderrLog"
    Write-Host ''

    Write-Step '启动 Tauri dev。'
    $env:DDNET_MANAGER_ALLOW_LOCAL_SMOKE = '1'
    $env:DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH = $resultPath
    $env:VITE_DDNET_MANAGER_LOCAL_SMOKE = '1'
    $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR = $clientInstallPath
    $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL = $manifestUrl
    $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH = '1'
    $env:CARGO_TARGET_DIR = $cargoTargetDir
    $env:APPDATA = $smokeAppData
    $env:LOCALAPPDATA = $smokeLocalAppData
    $env:XDG_DATA_HOME = $smokeXdgDataHome
    $env:XDG_CONFIG_HOME = $smokeXdgConfigHome
    $env:XDG_CACHE_HOME = $smokeXdgCacheHome
    Push-Location $root
    try {
        & bun run tauri dev --config $tauriDevConfigPath 1> $stdoutLog 2> $stderrLog
        if ($LASTEXITCODE -ne 0) {
            throw "bun run tauri dev exited with code $LASTEXITCODE. See $stderrLog"
        }
    }
    finally {
        Pop-Location
    }

    Write-Step '校验 smoke 结果文件。'
    Assert-LocalSmokeSucceeded $resultPath
    Write-Step '校验客户端文件已更新。'
    Assert-UpdatedClientBinary $clientInstallPath
    Write-Step '本地 smoke 自动更新验收通过。'
}
finally {
    if ($null -ne $listenerJob) {
        try {
            Stop-Job -Id $listenerJob.Id -ErrorAction SilentlyContinue
        }
        catch {
        }
        try {
            Remove-Job -Id $listenerJob.Id -Force -ErrorAction SilentlyContinue
        }
        catch {
        }
    }

    if ($null -eq $previousSmokeEnv) {
        Remove-Item Env:DDNET_MANAGER_ALLOW_LOCAL_SMOKE -ErrorAction SilentlyContinue
    }
    else {
        $env:DDNET_MANAGER_ALLOW_LOCAL_SMOKE = $previousSmokeEnv
    }

    if ($null -eq $previousSmokeResultPath) {
        Remove-Item Env:DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH -ErrorAction SilentlyContinue
    }
    else {
        $env:DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH = $previousSmokeResultPath
    }

    if ($null -eq $previousCargoTargetDir) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_TARGET_DIR = $previousCargoTargetDir
    }

    if ($null -eq $previousAppData) {
        Remove-Item Env:APPDATA -ErrorAction SilentlyContinue
    }
    else {
        $env:APPDATA = $previousAppData
    }

    if ($null -eq $previousLocalAppData) {
        Remove-Item Env:LOCALAPPDATA -ErrorAction SilentlyContinue
    }
    else {
        $env:LOCALAPPDATA = $previousLocalAppData
    }

    if ($null -eq $previousXdgDataHome) {
        Remove-Item Env:XDG_DATA_HOME -ErrorAction SilentlyContinue
    }
    else {
        $env:XDG_DATA_HOME = $previousXdgDataHome
    }

    if ($null -eq $previousXdgConfigHome) {
        Remove-Item Env:XDG_CONFIG_HOME -ErrorAction SilentlyContinue
    }
    else {
        $env:XDG_CONFIG_HOME = $previousXdgConfigHome
    }

    if ($null -eq $previousXdgCacheHome) {
        Remove-Item Env:XDG_CACHE_HOME -ErrorAction SilentlyContinue
    }
    else {
        $env:XDG_CACHE_HOME = $previousXdgCacheHome
    }

    if ($null -eq $previousViteLocalSmoke) {
        Remove-Item Env:VITE_DDNET_MANAGER_LOCAL_SMOKE -ErrorAction SilentlyContinue
    }
    else {
        $env:VITE_DDNET_MANAGER_LOCAL_SMOKE = $previousViteLocalSmoke
    }

    if ($null -eq $previousViteSmokeClientInstallDir) {
        Remove-Item Env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR = $previousViteSmokeClientInstallDir
    }

    if ($null -eq $previousViteSmokeManifestUrl) {
        Remove-Item Env:VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL -ErrorAction SilentlyContinue
    }
    else {
        $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL = $previousViteSmokeManifestUrl
    }

    if ($null -eq $previousViteSmokeCloseWindow) {
        Remove-Item Env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH -ErrorAction SilentlyContinue
    }
    else {
        $env:VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH = $previousViteSmokeCloseWindow
    }
}
