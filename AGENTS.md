# Contexto de trabajo

Lee DESIGN.md antes de modificar la interfaz. El usuario quiere un cambio visual importante y ha rechazado la primera versión por básica: comprueba el resultado renderizado, no solo el código.

La dirección confirmada más reciente es cinematográfica e inmersiva, estilo Infuse / Apple TV, con componentes hechos a medida. Ha rechazado también la versión con barra lateral azul. Conserva sus avatares originales y sus encuadres. PRODUCT.md recoge las decisiones del usuario.

Mantén la lógica de la aplicación en Rust. Usa los datos e imágenes de Jellyfin; no inventes títulos, progreso ni estadísticas. Trabaja en esta carpeta, sin modificar el proyecto hermano megalodon-front. El usuario ha autorizado ahora la aplicación nativa de Windows, la reproducción y un instalador; sustituye el aplazamiento anterior.

Para revisar código: `./dev.ps1 -Action test` y `./dev.ps1 -Action lint`. Para abrir la base local: `./dev.ps1 -Release` y http://127.0.0.1:3210.

Las credenciales permanecen en el proceso Rust. No se copian a assets, HTML, URLs, logs ni al repositorio.
