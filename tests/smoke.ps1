param([string]$BaseUrl = 'http://127.0.0.1:3210')
$ErrorActionPreference = 'Stop'

$catalog = Invoke-RestMethod "$BaseUrl/api/catalog"
if ($catalog.total -ne $catalog.items.Count) { throw 'El total no coincide con los títulos recibidos.' }
if (($catalog.items.Id | Sort-Object -Unique).Count -ne $catalog.total) { throw 'Hay títulos duplicados.' }
if ($catalog.movies + $catalog.series -ne $catalog.total) { throw 'Los tipos no cuadran con el total.' }

$pages = @('/', '/library', '/movies', '/series', '/connection')
foreach ($item in $catalog.items) { $pages += '/item/' + $item.Id }
foreach ($path in $pages) {
    $response = Invoke-WebRequest "$BaseUrl$path" -UseBasicParsing -TimeoutSec 30
    if ($response.StatusCode -ne 200 -or $response.Content -notmatch '<h1') { throw "Ficha incompleta: $path" }
    $pageScripts = [regex]::Matches($response.Content, '<script[^>]*>.*?</script>')
    if ($pageScripts.Count -ne 1 -or $pageScripts[0].Value -ne '<script type="module" src="/assets/boot.js?v=1"></script>') { throw "Script no autorizado: $path" }
    if ($response.Content -match 'api_key=|Token=|JELLYFIN_ADMIN_API_KEY') { throw "Credenciales expuestas: $path" }
    if ($response.Headers['Content-Security-Policy'] -notmatch "script-src 'self' 'wasm-unsafe-eval';" -or $response.Headers['Content-Security-Policy'] -match "'unsafe-inline'|'unsafe-eval'") { throw 'Política de scripts incorrecta.' }
    if ([regex]::Matches($response.Content, 'class="episode"').Count -gt 40) { throw 'La temporada no está paginada.' }
}

foreach ($headers in @(@{ Host = 'untrusted.invalid' }, @{ 'Sec-Fetch-Site' = 'cross-site' })) {
    try { Invoke-WebRequest "$BaseUrl/api/catalog" -Headers $headers -UseBasicParsing | Out-Null; throw 'Se ha permitido un origen externo.' }
    catch { if ([int]$_.Exception.Response.StatusCode -ne 403) { throw } }
}
try { Invoke-WebRequest "$BaseUrl/refresh" -Method Post -Headers @{ Origin = 'https://untrusted.invalid' } -UseBasicParsing | Out-Null; throw 'Se ha permitido una actualización desde otro origen.' }
catch { if ([int]$_.Exception.Response.StatusCode -ne 403) { throw } }

$imageItem = $catalog.items | Where-Object { $_.ImageTags.Primary } | Select-Object -First 1
if ($imageItem) {
    $image = Invoke-WebRequest "$BaseUrl/image/$($imageItem.Id)/Primary?width=240" -UseBasicParsing
    if ($image.Headers['Content-Type'] -notmatch 'image/(webp|png|jpeg|avif)') { throw 'La portada no se ha recibido de Jellyfin.' }
}
[pscustomobject]@{ Result = 'OK'; Titles = $catalog.total; Movies = $catalog.movies; Series = $catalog.series; Pages = $pages.Count; OriginGuards = 'OK'; ImageProxy = 'OK' }
$avatar = Invoke-WebRequest "$BaseUrl/profile/avatar" -UseBasicParsing
if ($avatar.StatusCode -ne 200 -or $avatar.Headers['Content-Type'] -notmatch 'image/(webp|png|jpeg|avif|svg\+xml)') { throw 'El avatar del perfil no se recibe.' }
$homeMarkup = (Invoke-WebRequest "$BaseUrl/" -UseBasicParsing).Content
if ($homeMarkup -notmatch 'popovertarget="profile-panel"' -or $homeMarkup -notmatch '/profile/avatar') { throw 'Falta el componente de perfil.' }
Write-Output 'Perfil y menús: OK'
$wasm = Invoke-WebRequest "$BaseUrl/assets/megalodon_ui_bg.wasm" -UseBasicParsing
if ($wasm.Headers['Content-Type'] -ne 'application/wasm' -or $wasm.Content.Length -gt 200000) { throw 'El módulo de interacción no es válido o supera el presupuesto.' }
if ($homeMarkup -notmatch 'aria-controls="recent-rail"' -or $homeMarkup -notmatch 'data-media') { throw 'Faltan carruseles o estados de carga.' }
Write-Output 'Interacción Rust/Wasm: OK'
