# Bump the latest vX.Y.Z[-rcN] git tag and apply the new tag to the current commit.
# With -Rc, the new tag is a release candidate (vX.Y.Z-rcN). If the latest tag is
# an rc and no -Major/-Minor/-Patch is given, -Rc increments N, and a run
# without -Rc promotes it to the final vX.Y.Z.
param(
    [switch]$Major,
    [switch]$Minor,
    [switch]$Patch,
    [switch]$Rc
)

$ErrorActionPreference = "Stop"

if ($Major) { $bump = "major" }
elseif ($Minor) { $bump = "minor" }
elseif ($Patch) { $bump = "patch" }
else { $bump = $null }

$pattern = '^v(\d+\.\d+\.\d+)(?:-rc(\d+))?$'

# A final release sorts after its release candidates.
$latestTag = git tag --list 'v*.*.*' | Where-Object { $_ -match $pattern } |
    Sort-Object { [version]([regex]::Match($_, $pattern).Groups[1].Value) },
        { $m = [regex]::Match($_, $pattern).Groups[2]; if ($m.Success) { [int]$m.Value } else { [int]::MaxValue } } |
    Select-Object -Last 1
if (-not $latestTag) { $latestTag = "v0.0.0" }

$match = [regex]::Match($latestTag, $pattern)
$version = [version]$match.Groups[1].Value
$rcNum = if ($match.Groups[2].Success) { [int]$match.Groups[2].Value } else { $null }
$verMajor = $version.Major
$verMinor = $version.Minor
$verPatch = $version.Build
if ($verPatch -lt 0) { $verPatch = 0 }

$newRc = $null
if ($null -ne $rcNum -and -not $bump) {
    # Continue or finalize the pending release candidate series.
    if ($Rc) { $newRc = $rcNum + 1 }
}
else {
    if (-not $bump) { $bump = "patch" }
    switch ($bump) {
        "major" { $verMajor++; $verMinor = 0; $verPatch = 0 }
        "minor" { $verMinor++; $verPatch = 0 }
        "patch" { $verPatch++ }
    }
    if ($Rc) { $newRc = 1 }
}

$newTag = "v$verMajor.$verMinor.$verPatch"
if ($null -ne $newRc) { $newTag += "-rc$newRc" }

git tag -a $newTag -m $newTag
Write-Host "Tagged current commit as $newTag (previous: $latestTag)"
Write-Host "Push with: git push origin $newTag"
