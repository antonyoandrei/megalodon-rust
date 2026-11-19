# Megalodon

Cliente de escritorio para Windows para ver tu propia biblioteca de películas y series: estilo cine, perfiles por persona, Seguir viendo y reproductor integrado. Escrito en Rust (interfaz nativa con egui, vídeo con libmpv).

> **Uso personal.** Puedes usarlo, estudiarlo y hacer un fork para ti. No se aceptan contribuciones a este repositorio y no se permite el uso comercial. Consulta [Licencia](#licencia).

## Instalar

1. Descarga `Megalodon-<versión>-windows-x64-setup.exe` desde [Releases](../../releases).
2. Ábrelo y pulsa **Instalar Megalodon**. Se instala solo para tu usuario, sin permisos de administrador.
3. Al abrir Megalodon por primera vez, un asistente te pide tu nombre, un tema y la biblioteca que quieres conectar.

También hay una versión portable (`.zip`): descomprímela y abre `Megalodon.exe`.

Requisitos: Windows 10 u 11 de 64 bits.

## Conectar tu biblioteca

Puedes conectar varias bibliotecas. Se gestionan en **Ajustes (engranaje) → Conexiones → Añadir conexión**, o desde el último paso del asistente.

### Jellyfin

Úsalo si ya tienes un servidor [Jellyfin](https://jellyfin.org) con tu biblioteca.

1. **Ajustes → Añadir conexión → Jellyfin.**
2. **Servidor**: la dirección completa, por ejemplo `http://192.168.1.10:8096` en tu red o `https://jellyfin.tudominio.com` desde fuera.
3. **Usuario** y **Contraseña** de tu cuenta de Jellyfin. No hace falta una cuenta de administrador.
4. Pulsa **Conectar**.

Megalodon guarda una sesión cifrada con Windows (DPAPI), nunca tu contraseña. No modifica la configuración del servidor.

### Servidor privado (NAS, Nextcloud o WebDAV)

Úsalo si tus vídeos están en un NAS (Synology, QNAP…), en Nextcloud o en cualquier servidor que ofrezca **WebDAV**. No necesitas instalar nada en el servidor.

1. Activa WebDAV en tu servidor y anota la dirección de la carpeta de vídeos. Ejemplos:
   - Synology: `https://nas.local:5006/video/`
   - Nextcloud: `https://nube.tudominio.com/remote.php/dav/files/USUARIO/Peliculas/`
2. **Ajustes → Añadir conexión → WebDAV.**
3. **Dirección de la carpeta**, **Usuario** y **Contraseña** del servidor.
4. Pulsa **Conectar**. Megalodon comprueba el acceso y lee la biblioteca.

La contraseña se guarda cifrada con DPAPI y solo se usa para leer. Megalodon nunca borra ni modifica tus vídeos.

### Cómo organizar los archivos

Funciona con la estructura habitual de Jellyfin, Kodi o Plex:

```
Películas/
  Título (Año)/
    Título (Año).mkv
    Título (Año).nfo        opcional
Series/
  Nombre de la serie/
    Temporada 1/
      Nombre - S01E01 - Título.mkv
```

Los archivos `.nfo` (títulos, sinopsis, IDs) y las imágenes junto al vídeo (`poster.jpg`, `fanart.jpg`, `logo.png`) se usan si existen.

## Imágenes y fichas

Al conectar una biblioteca, Megalodon completa solo los carteles, fondos y logotipos que falten, sin cuentas: Cinemeta/Metahub, TVmaze, AniList y Kitsu. TMDB y fanart.tv son opcionales (**Ajustes → Imágenes y fichas**).

En la ficha de cada título, el botón de imágenes te deja elegir otro cartel, fondo o logotipo, o **subir el tuyo**.

### Tus propias imágenes

Dentro de tu biblioteca, una carpeta `posters` (también vale `images`, `artwork`, `covers` o `portadas`) con una subcarpeta por título:

```
posters/
  Título (Año)/
    poster.jpg
    background.jpg
    logo.png
  avatars/
    Categoría/
      Nombre.png
```

Tus imágenes tienen prioridad sobre todo lo demás. Las de `avatars/` aparecen en la galería de avatares, agrupadas por categoría.

## Perfiles, temas y privacidad

- **Perfiles** sin contraseña: cada persona tiene su progreso, favoritos y Seguir viendo. Sin foto elegida, cada perfil recibe un color distinto.
- **Temas** en **Ajustes → Apariencia**: Cine, Abisal, Ámbar, Nebulosa y Negro puro.
- **Tus datos** viven solo en tu equipo, en `%LOCALAPPDATA%\MegalodonRust`. Desinstalar no los borra. No hay telemetría, y a los proveedores de imágenes no se les envían perfiles, progreso ni favoritos.

## Compilar desde el código

Requisitos: Rust estable, Windows 10/11 x64, Python 3 y el compilador de recursos (`rc` o `windres`).

```powershell
.\scripts\setup-mpv.ps1
.\scripts\setup-rclone.ps1
.\scripts\setup-vulkan.ps1
.\desktop.ps1 -Action run            # ejecutar
.\desktop.ps1 -Action test           # pruebas
.\desktop.ps1 -Action lint           # clippy
.\package.ps1                        # instalador y portable en dist/
```

Claves opcionales del proyecto (TMDB, fanart.tv): créalas en `.local/providers.env` (ignorado por git) o como variables de entorno al compilar:

```
MEGALODON_TMDB_TOKEN=...
MEGALODON_FANART_KEY=...
```

En GitHub Actions, añádelas como *repository secrets* con esos mismos nombres. Nunca las subas al repositorio.

Más detalle: [escritorio](docs/DESKTOP.md), [conexiones](docs/CONNECTIONS.md), [distribución](docs/DISTRIBUTION.md), [terceros](docs/THIRD-PARTY.md).

## Licencia

[PolyForm Noncommercial 1.0.0](LICENSE). En resumen:

- **Puedes** usarlo, modificarlo y hacer un fork para uso personal o sin ánimo de lucro.
- **No puedes** usarlo con fines comerciales.
- **No se aceptan** pull requests ni contribuciones a este repositorio: se cierran automáticamente. Mira [CONTRIBUTING](CONTRIBUTING.md).

Los componentes de terceros (libmpv, rclone, cargador Vulkan, fuentes, crates de Rust) mantienen sus propias licencias: [docs/THIRD-PARTY.md](docs/THIRD-PARTY.md) y [docs/licenses](docs/licenses). Este producto usa la API de TMDB, pero TMDB no lo respalda ni lo certifica.
