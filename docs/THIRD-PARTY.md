# Componentes de terceros

El código propio de Megalodon usa MIT. Esta licencia no sustituye las licencias de dependencias, imágenes de perfil, marcas ni contenido de las bibliotecas del usuario.

## Motor multimedia de esta compilación local

- libmpv: https://github.com/mpv-player/mpv, revisión `bcde6b93cc`.
- Binario Windows: https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20261111
- Receta y configuración: https://github.com/shinchiro/mpv-winbuild-cmake
- `mpv-build.json` identifica el archivo original y su SHA-256.
- Condiciones de mpv: https://github.com/mpv-player/mpv/blob/master/Copyright
- FFmpeg: https://ffmpeg.org/legal.html

mpv puede compilarse bajo GPLv2+ o LGPLv2.1+ según sus opciones y dependencias. Este paquete utiliza el binario completo de shinchiro; no se presenta como una compilación exclusivamente LGPL. Se incluyen las licencias de referencia. La configuración exacta y el código fuente correspondiente de todas las dependencias de esa compilación deben acompañar una futura distribución pública; el archivo fuente de Megalodon no sustituye el código de mpv/FFmpeg.

## Dependencias Rust

El paquete incorpora rclone 1.76.0, herramienta de conexión de Google Drive bajo licencia MIT. Procede de la publicación oficial `rclone/rclone` y se verifica el ZIP Windows amd64 contra SHA-256 `8b4fd2737ade4e70c057ef4a49deb7cffe94b0fb9d7587c189d149823f21a1dc`. Se incluye su README con los créditos y condiciones. No se modifican archivos de Drive ni se necesita montar una unidad para la conexión directa.

Se incluye el cargador Vulkan oficial de LunarG 1.4.357.0, requerido al cargar esta compilación de libmpv aunque la salida sea OpenGL. Archivo original: https://sdk.lunarg.com/sdk/download/1.4.357.0/windows/VulkanRT-X64-1.4.357.0-Components.zip, SHA-256 `a14672efed15aafc7f5a16572d35cd3a3416eadf670aeee3cdf50ee32d5fbf83`. Su licencia acompaña al paquete. No se instalan controladores ni se reemplazan archivos del sistema.

`Cargo.lock` fija las versiones. `rust-dependencies.json` registra nombre, versión, licencia declarada y repositorio. Los textos de licencia incluidos en los paquetes descargados se copian bajo `rust/`.

## Recursos visuales

El arte del catálogo se obtiene del servidor del usuario y no se incorpora al instalador. Los presets de avatar y la marca se conservan del proyecto original para la prueba local; sus derechos no se transfieren por la licencia MIT del código. Un paquete público genérico debe utilizar recursos propios o con permiso de redistribución.

## Metadatos opcionales

TVmaze: datos bajo CC BY-SA, con atribución en Ajustes. https://www.tvmaze.com/api
TMDB: token de lectura propio, marca oficial y atribución incluidas. https://developer.themoviedb.org/docs/faq
fanart.tv: clave personal propia y atribución incluida. https://api.fanart.tv/
Las imágenes descargadas se almacenan en los datos privados del equipo y no se distribuyen dentro del programa.
