# Script that generates temporary dummy icons.
# 1x1 transparent PNG, base64-encoded.
$pngBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
$pngBytes = [Convert]::FromBase64String($pngBase64)

# Write the PNGs.
[System.IO.File]::WriteAllBytes("$PSScriptRoot\32x32.png", $pngBytes)
[System.IO.File]::WriteAllBytes("$PSScriptRoot\128x128.png", $pngBytes)
[System.IO.File]::WriteAllBytes("$PSScriptRoot\128x128@2x.png", $pngBytes)

# ICO and ICNS placeholders (text-only stand-ins).
"ICO placeholder" | Out-File -FilePath "$PSScriptRoot\icon.ico" -Encoding ASCII
"ICNS placeholder" | Out-File -FilePath "$PSScriptRoot\icon.icns" -Encoding ASCII

Write-Host "Icons created successfully!"
