$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot
$brand = Join-Path $root 'static/brand'

# Master: megalodon-simbolo-blanco.svg (absolute M/L/C/Q/Z path inside an optional
# rotate() group). Rendered on transparency, fitted and centred in a square.
# Everything is the white symbol, as inside the app (the person's choice): window,
# taskbar, shortcuts, installer and navigation.
[xml]$svg = Get-Content -LiteralPath (Join-Path $brand 'megalodon-simbolo-blanco.svg') -Raw -Encoding UTF8
$ns = New-Object Xml.XmlNamespaceManager($svg.NameTable)
$ns.AddNamespace('s', 'http://www.w3.org/2000/svg')
$pathNode = $svg.SelectSingleNode('//s:path', $ns)
$group = $svg.SelectSingleNode('//s:g', $ns)

function New-LogoPath([string]$d) {
    $tokens = [regex]::Matches($d, '[MLCQZ]|-?\d+(?:\.\d+)?') | ForEach-Object Value
    $path = New-Object Drawing.Drawing2D.GraphicsPath
    $path.FillMode = [Drawing.Drawing2D.FillMode]::Alternate
    $i = 0; $x = 0.0; $y = 0.0; $command = 'M'
    while ($i -lt $tokens.Count) {
        if ($tokens[$i] -match '^[MLCQZ]$') { $command = $tokens[$i++] }
        switch ($command) {
            'M' { $path.StartFigure(); $x = [single]$tokens[$i++]; $y = [single]$tokens[$i++]; $command = 'L' }
            'L' { $nx = [single]$tokens[$i++]; $ny = [single]$tokens[$i++]; $path.AddLine([single]$x, [single]$y, $nx, $ny); $x = $nx; $y = $ny }
            'C' {
                $x1 = [single]$tokens[$i++]; $y1 = [single]$tokens[$i++]; $x2 = [single]$tokens[$i++]; $y2 = [single]$tokens[$i++]
                $nx = [single]$tokens[$i++]; $ny = [single]$tokens[$i++]
                $path.AddBezier([single]$x, [single]$y, $x1, $y1, $x2, $y2, $nx, $ny); $x = $nx; $y = $ny
            }
            'Q' {
                # Quadratic to cubic: control points at 2/3 towards the quadratic control.
                $qx = [single]$tokens[$i++]; $qy = [single]$tokens[$i++]; $nx = [single]$tokens[$i++]; $ny = [single]$tokens[$i++]
                $c1x = $x + 2 / 3 * ($qx - $x); $c1y = $y + 2 / 3 * ($qy - $y)
                $c2x = $nx + 2 / 3 * ($qx - $nx); $c2y = $ny + 2 / 3 * ($qy - $ny)
                $path.AddBezier([single]$x, [single]$y, [single]$c1x, [single]$c1y, [single]$c2x, [single]$c2y, $nx, $ny); $x = $nx; $y = $ny
            }
            'Z' { $path.CloseFigure(); $command = 'M' }
        }
    }
    if ($group -and $group.transform -match 'rotate\(\s*(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s*\)') {
        $matrix = New-Object Drawing.Drawing2D.Matrix
        $matrix.RotateAt([single]$Matches[1], (New-Object Drawing.PointF([single]$Matches[2], [single]$Matches[3])))
        $path.Transform($matrix); $matrix.Dispose()
    }
    return , $path
}

function Render([int]$size, [string]$color, [double]$fill = 0.9) {
    $path = New-LogoPath $pathNode.d
    $bounds = $path.GetBounds()
    $scale = $size * $fill / [Math]::Max($bounds.Width, $bounds.Height)
    $bitmap = New-Object Drawing.Bitmap($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([Drawing.Color]::Transparent)
    $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.TranslateTransform([single]($size / 2), [single]($size / 2))
    $graphics.ScaleTransform([single]$scale, [single]$scale)
    $graphics.TranslateTransform([single](-($bounds.X + $bounds.Width / 2)), [single](-($bounds.Y + $bounds.Height / 2)))
    $brush = New-Object Drawing.SolidBrush([Drawing.ColorTranslator]::FromHtml($color))
    $graphics.FillPath($brush, $path)
    $stream = New-Object IO.MemoryStream
    $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray()
    $brush.Dispose(); $path.Dispose(); $graphics.Dispose(); $bitmap.Dispose(); $stream.Dispose()
    return , $bytes
}

$white = '#FFFFFF'
[IO.File]::WriteAllBytes((Join-Path $brand 'mark.png'), (Render 512 $white))
[IO.File]::WriteAllBytes((Join-Path $brand 'mark-white.png'), (Render 512 '#FFFFFF'))
$payloads = foreach ($size in @(16, 24, 32, 48, 64, 128, 256)) {
    # Small sizes fill a little more of the square so the silhouette stays legible.
    @{ Size = $size; Bytes = (Render $size $white $(if ($size -le 32) { 0.98 } else { 0.92 })) }
}
$stream = [IO.File]::Create((Join-Path $root 'static/megalodon.ico'))
$writer = New-Object IO.BinaryWriter($stream)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$payloads.Count)
$offset = 6 + 16 * $payloads.Count
foreach ($entry in $payloads) {
    $dimension = if ($entry.Size -eq 256) { 0 } else { $entry.Size }
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension); $writer.Write([uint16]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32); $writer.Write([uint32]$entry.Bytes.Length); $writer.Write([uint32]$offset)
    $offset += $entry.Bytes.Length
}
foreach ($entry in $payloads) { $writer.Write([byte[]]$entry.Bytes) }
$writer.Dispose(); $stream.Dispose()
Write-Output 'Rendered Megalodon symbol: mark.png, mark-white.png and megalodon.ico (16-256 px).'
