$iconSize = 32
$canvas = New-Object System.Drawing.Bitmap($iconSize, $iconSize)
$graphics = [System.Drawing.Graphics]::FromImage($canvas)
$graphics.Clear([System.Drawing.Color]::Transparent)

$brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(139, 92, 246))
$graphics.FillRectangle($brush, 4, 4, 24, 24)

$pen = New-Object System.Drawing.Pen([System.Drawing.Color]::White, 2)
$graphics.DrawLine($pen, 8, 12, 24, 12)
$graphics.DrawLine($pen, 8, 16, 20, 16)
$graphics.DrawLine($pen, 8, 20, 16, 20)

$canvas.Save("src-tauri/icons/tray.png", [System.Drawing.Imaging.ImageFormat]::Png)

$canvas.Dispose()
$graphics.Dispose()
$brush.Dispose()
$pen.Dispose()

Write-Host "Icons generated successfully"
