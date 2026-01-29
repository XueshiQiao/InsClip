param (
    [string]$Type = "patch"
)

# 1. Bump package.json version
Write-Host "Bumping package.json version ($Type)..."
npm version $Type --no-git-tag-version

if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to bump npm version"
    exit 1
}

# 2. Read the new version
$subVersion = (Get-Content -Path .\package.json | ConvertFrom-Json).version
Write-Host "New version is: $subVersion"

# 3. Update tauri.conf.json
Write-Host "Updating tauri.conf.json..."
$tauriConfPath = ".\src-tauri\tauri.conf.json"
$tauriJson = Get-Content -Path $tauriConfPath -Raw | ConvertFrom-Json
$tauriJson.version = $subVersion
$tauriJson.productName = "PastePaw" # Ensure consistent formatting if needed, though mostly reading/writing
$tauriJson | ConvertTo-Json -Depth 100 | Set-Content -Path $tauriConfPath

# 4. Update Cargo.toml
Write-Host "Updating Cargo.toml..."
$cargoTomlPath = ".\src-tauri\Cargo.toml"
(Get-Content -Path $cargoTomlPath) -replace '^version = ".*"', ('version = "' + $subVersion + '"') | Set-Content -Path $cargoTomlPath

Write-Host "Version bumped to $subVersion in all files."
Write-Host "You can now commit and tag: git commit -am 'v$subVersion' && git tag v$subVersion"
