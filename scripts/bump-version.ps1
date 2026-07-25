# Bump the latest vX.Y.Z git tag and apply the new tag to the current commit.
param(
    [switch]$Major,
    [switch]$Minor,
    [switch]$Patch
)

$ErrorActionPreference = "Stop"

if ($Major) { $bump = "major" }
elseif ($Minor) { $bump = "minor" }
else { $bump = "patch" }

$latestTag = git tag --list 'v*.*.*' | Sort-Object { [version]($_ -replace '^v', '') } | Select-Object -Last 1
if (-not $latestTag) { $latestTag = "v0.0.0" }

$version = [version]($latestTag -replace '^v', '')
$verMajor = $version.Major
$verMinor = $version.Minor
$verPatch = $version.Build
if ($verPatch -lt 0) { $verPatch = 0 }

switch ($bump) {
    "major" { $verMajor++; $verMinor = 0; $verPatch = 0 }
    "minor" { $verMinor++; $verPatch = 0 }
    "patch" { $verPatch++ }
}

$newTag = "v$verMajor.$verMinor.$verPatch"

git tag -a $newTag -m $newTag
Write-Host "Tagged current commit as $newTag (previous: $latestTag)"
Write-Host "Push with: git push origin $newTag"
