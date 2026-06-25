#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$GithubRepo = if ($env:CHOBITS_GITHUB_REPO) { $env:CHOBITS_GITHUB_REPO } else { "NewComer00/Chobits" }
$InstallDir = if ($env:CHOBITS_INSTALL_DIR) {
    $env:CHOBITS_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "Chobits"
}
$Version = if ($env:CHOBITS_VERSION) { $env:CHOBITS_VERSION } else { "latest" }
$BinDir = if ($env:CHOBITS_BIN_DIR) { $env:CHOBITS_BIN_DIR } else { Join-Path $InstallDir "bin" }

$script:Ansi = @{
    R  = ''
    B  = ''
    D  = ''
    C  = ''
    BC = ''
    BY = ''
    BG = ''
}

function Enable-VirtualTerminal {
    if ($env:OS -notmatch 'Windows') { return }
    try {
        $sig = @'
[DllImport("kernel32.dll")] public static extern int GetStdHandle(int h);
[DllImport("kernel32.dll")] public static extern bool GetConsoleMode(int h, out int m);
[DllImport("kernel32.dll")] public static extern bool SetConsoleMode(int h, int m);
'@
        $vt = Add-Type -MemberDefinition $sig -Name ChobitsVT -PassThru -ErrorAction Stop
        $h = $vt::GetStdHandle(-11)
        $m = 0
        [void]$vt::GetConsoleMode($h, [ref]$m)
        [void]$vt::SetConsoleMode($h, $m -bor 4)
    } catch {
        # Windows Terminal and redirected hosts may not need this.
    }
}

function Initialize-Ansi {
    if ($env:NO_COLOR -or [Console]::IsOutputRedirected) { return }
    Enable-VirtualTerminal
    $e = [char]27
    $script:Ansi.R = "${e}[0m"
    $script:Ansi.B = "${e}[1m"
    $script:Ansi.D = "${e}[2m"
    $script:Ansi.C = "${e}[36m"
    $script:Ansi.BC = "${e}[1;36m"
    $script:Ansi.BY = "${e}[1;33m"
    $script:Ansi.BG = "${e}[1;32m"
}

Initialize-Ansi

function Write-Info([string]$Message) {
    Write-Host "chobits: $Message"
}

function Write-Success([string]$Message) {
    if ($script:Ansi.BG) {
        Write-Host "chobits: $($script:Ansi.BG)$Message$($script:Ansi.R)"
    } else {
        Write-Host "chobits: $Message"
    }
}

function Show-DownloadProgress {
    param(
        [string]$Label,
        [long]$Done,
        [long]$Total
    )

    if ([Console]::IsOutputRedirected) { return }

    $a = $script:Ansi
    $width = 32
    if ($Total -gt 0) {
        $pct = [Math]::Min(100, [int](100 * $Done / $Total))
        $filled = [int]($width * $pct / 100)
        $bar = ('=' * $filled).PadRight($width, '-')
        $mb = '{0:N1}' -f ($Done / 1MB)
        $totalMb = '{0:N1}' -f ($Total / 1MB)
        $line = "  [$bar]  ${pct}%  ${mb} / ${totalMb} MB"
    } else {
        $mb = '{0:N1}' -f ($Done / 1MB)
        $bar = ('-' * $width)
        $line = "  [$bar]  ${mb} MB"
    }

    if ($a.C) {
        Write-Host -NoNewline ("`r$($a.D)chobits: $($a.C)$Label$($a.R) $line$($a.R)")
    } else {
        Write-Host -NoNewline ("`rchobits: $Label $line")
    }
}

function Clear-DownloadProgress {
    if ([Console]::IsOutputRedirected) { return }
    Write-Host ("`r" + (' ' * 100) + "`r") -NoNewline
}

function Save-Download {
    param(
        [Parameter(Mandatory)]
        [string]$Url,
        [Parameter(Mandatory)]
        [string]$DestinationPath,
        [Parameter(Mandatory)]
        [string]$Label
    )

    if ([Console]::IsOutputRedirected) {
        Invoke-WebRequest -Uri $Url -OutFile $DestinationPath -UseBasicParsing
        return
    }

    $request = [System.Net.HttpWebRequest]::Create($Url)
    $request.UserAgent = 'chobits-installer'
    $request.AllowAutoRedirect = $true
    $response = $request.GetResponse()
    try {
        $total = $response.ContentLength
        $input = $response.GetResponseStream()
        $output = [System.IO.File]::Open($DestinationPath, [System.IO.FileMode]::Create)
        try {
            $buffer = New-Object byte[] 65536
            $downloaded = 0L
            while (($read = $input.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $output.Write($buffer, 0, $read)
                $downloaded += $read
                Show-DownloadProgress -Label $Label -Done $downloaded -Total $total
            }
        } finally {
            $output.Close()
            $input.Close()
        }
    } finally {
        $response.Close()
        Clear-DownloadProgress
    }
}

function Add-ToUserPath([string]$PathToAdd) {
    if ($env:CHOBITS_NO_MODIFY_PATH -eq "1") {
        Write-Info "skipped PATH update (CHOBITS_NO_MODIFY_PATH=1)"
        return
    }

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($userPath) {
        $parts = $userPath -split ';' | Where-Object { $_ -and ($_ -ne $PathToAdd) }
    }
    $newPath = (@($PathToAdd) + $parts) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    $env:Path = "$PathToAdd;$env:Path"
    Write-Info "added $PathToAdd to user PATH"
}

function Show-NextSteps([string]$ConfigPath) {
    $a = $script:Ansi
    Write-Host ""
    Write-Host "  $($a.BC)Next steps$($a.R)"
    Write-Host "  $($a.D)----------$($a.R)"
    Write-Host ""
    Write-Host "  $($a.B)1.$($a.R) Edit $($a.BY)[llm]$($a.R) in $($a.C)$ConfigPath$($a.R)"
    Write-Host "     pick one example:"
    Write-Host ""
    Write-Host "     $($a.BY)Ollama:$($a.R)"
    Write-Host @"
     [llm]
     backend    = "ollama"
     url        = "http://localhost:11434"
     model      = "qwen3:0.6b"
     max_tokens = 512
"@
    Write-Host ""
    Write-Host "     $($a.BY)OpenAI-compatible API:$($a.R)"
    Write-Host @"
     [llm]
     backend    = "deepseek"
     url        = "https://api.deepseek.com"
     model      = "deepseek-v4-flash"
     max_tokens = 512
     api_key    = "sk-..."
"@
    Write-Host ""
    Write-Host "  $($a.B)2.$($a.R) $($a.D)(optional, Ollama only)$($a.R) install https://ollama.com, then run"
    Write-Host "     $($a.BG)ollama pull qwen3:0.6b$($a.R)"
    Write-Host ""
    Write-Host "  $($a.B)3.$($a.R) Launch:"
    Write-Host "     $($a.BG)chobits-start$($a.R)"
    Write-Host ""
}

if ($env:PROCESSOR_ARCHITECTURE -notmatch "64") {
    throw "chobits: unsupported architecture (x86_64 only)"
}

$target = "x86_64-pc-windows-gnu"
$archive = "Chobits-$target.zip"

if ($Version -eq "latest") {
    $base = "https://github.com/$GithubRepo/releases/latest/download"
} else {
    $base = "https://github.com/$GithubRepo/releases/download/$Version"
}

$url = "$base/$archive"
$tmp = Join-Path $env:TEMP ("chobits-install-" + [guid]::NewGuid().ToString())
$zipPath = Join-Path $tmp $archive
$extractDir = Join-Path $tmp "extract"

New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
    Save-Download -Url $url -DestinationPath $zipPath -Label "downloading $archive"

    Write-Info "installing to $InstallDir"
    if (Test-Path $InstallDir) {
        Remove-Item -Recurse -Force $InstallDir
    }

    $installParent = Split-Path $InstallDir -Parent
    if ($installParent) {
        New-Item -ItemType Directory -Path $installParent -Force | Out-Null
    }

    New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    $root = Join-Path $extractDir "Chobits"
    if (-not (Test-Path $root)) {
        throw "chobits: archive did not contain a Chobits/ folder"
    }

    Move-Item -Path $root -Destination $InstallDir
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Add-ToUserPath $BinDir

Write-Success "installed to $InstallDir"
Show-NextSteps (Join-Path $InstallDir "config.toml")
