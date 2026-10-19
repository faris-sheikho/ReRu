param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    if (-not $SkipBuild) {
        cargo build --locked --release
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package metadata' }
    $version = ($metadata.packages | Where-Object name -eq 'reru').version
    $binary = Join-Path $repo 'target/release/reru.exe'
    $reported = & $binary --version
    if ($LASTEXITCODE -ne 0 -or $reported -ne "ReRu $version") {
        throw 'The release binary does not match Cargo.toml; rebuild first'
    }
    $output = Join-Path $repo 'target/releases'
    New-Item -ItemType Directory -Force -Path $output | Out-Null
    $archive = Join-Path $output "ReRu-$version-windows-x86_64.zip"
    $contents = @($binary, 'README.md', 'THIRD_PARTY_NOTICES.md', 'docs/COMPATIBILITY.md',
        'docs/ROADMAP.md', 'docs/RELEASE_NOTES.md',
        'docs/REA_WORKFLOW.md', 'docs/ReRu_Implementation_Plan.pdf')
    # Preserve documentation paths and README relative links in the archive.
    Add-Type -AssemblyName System.IO.Compression
    $file = [IO.File]::Open($archive, [IO.FileMode]::Create)
    $zip = [IO.Compression.ZipArchive]::new($file, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($source in $contents) {
            $name = if ($source -eq $binary) { 'reru.exe' } else { $source }
            $entry = $zip.CreateEntry($name)
            $destination = $entry.Open()
            $inputFile = [IO.File]::OpenRead((Resolve-Path -LiteralPath $source).Path)
            try { $inputFile.CopyTo($destination) }
            finally { $inputFile.Dispose(); $destination.Dispose() }
        }
    } finally { $zip.Dispose(); $file.Dispose() }
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" |
        Set-Content -LiteralPath "$archive.sha256" -Encoding ascii
    Write-Output $archive
    Write-Output "SHA256 $hash"
} finally { Pop-Location }
