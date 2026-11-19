# Megalodon Rust

Base local de un cliente personal pensado para publicarse como código abierto: interfaz, catálogo completo de Jellyfin y fichas de películas y series. Proyecto independiente de `../megalodon-front`.

El objetivo es que cada persona conecte sus bibliotecas y fuentes desde la aplicación. Hoy solo está implementado Jellyfin con configuración local; el asistente de conexión, la autenticación desde la interfaz y los complementos de Stremio siguen pendientes.

## Probar en este equipo

1. Mantén Jellyfin abierto y el montaje de la biblioteca disponible, si utilizas Drive.
2. Ejecuta en PowerShell:

   ```powershell
   .\dev.ps1 -Release
   ```

3. Abre **http://127.0.0.1:3210**. Ctrl+C en esa consola cierra la aplicación.

La versión preparada para esta sesión está en `target/release/megalodon.exe`. También se puede ejecutar directamente desde la carpeta del proyecto. La marca y los estilos están incluidos en el ejecutable; los presets de perfil se sirven desde `static/avatars` y el arte del catálogo desde Jellyfin. No requiere Node, Electron ni un proceso de compilación del frontend. Si ya hay una instancia abierta en ese puerto, utiliza su dirección en el navegador.

## Qué funciona

- Inicio con arte y logotipos reales, selección manual de destacados y estanterías. El hero cambia sin recargar, espera a decodificar la imagen y conserva la escena anterior durante la carga o si falla; prevalece la última selección.
- Películas, series y biblioteca general; búsqueda sin distinguir mayúsculas ni tildes.
- Filtros por género y tipo; orden por título, incorporación, año y valoración.
- Todas las páginas de Jellyfin, sin límite fijo de títulos. Catálogo en memoria durante cinco minutos y actualización manual.
- Fichas con sinopsis, metadatos, géneros, producción, dirección y reparto cuando existen en Jellyfin.
- Temporadas, especiales y episodios con imagen, duración y sinopsis desplegable; listas largas paginadas.
- Marcadores de visto procedentes de Jellyfin, sin modificar el historial.
- Navegación superior sobre el arte en escritorio y barra inferior en móvil, foco visible y movimiento reducido.
- Componentes propios de búsqueda, filtros, perfil y temporadas. Menús nativos accesibles con Escape, foco y transiciones de entrada y salida.
- Carruseles con extremos difuminados, flechas, teclado, desplazamiento suave y tarjetas completas por ancho disponible.
- Skeletons ligados a la carga de imágenes, transición a la imagen real y alternativa de error; se pausan fuera de pantalla. Fundidos de navegación y movimiento reducido.
- Avatares elegidos en el Megalodon original, con sus encuadres, y soporte para imágenes de usuario de Jellyfin.
- Estado de conexión, mensajes de error y conservación del catálogo anterior si una actualización falla.

La reproducción está aplazada expresamente. Esta entrega es una interfaz web local generada por Rust; todavía no es una aplicación nativa para Windows, móvil o TV. No se ha implementado acceso desde otros dispositivos.

## Configuración

No hace falta copiar las claves del proyecto anterior para probar en este equipo. Se leen en memoria:

1. Valores heredados de `../megalodon-front/.env.local`.
2. Clave vigente del launcher en `%LOCALAPPDATA%/Megalodon/secrets.json`.
3. Valores de `.env` y `.env.local` en esta carpeta.
4. Variables del entorno, con prioridad máxima.

Puedes crear `.env.local` a partir de `.env.example` y configurar `JELLYFIN_BASE_URL`, `JELLYFIN_API_KEY`, `JELLYFIN_USER_ID` y `MEGALODON_PORT`. Una clave explícita `JELLYFIN_API_KEY` tiene prioridad sobre `JELLYFIN_ADMIN_API_KEY`.

Sin `JELLYFIN_USER_ID`, se elige un perfil administrador habilitado, o el primer perfil activo si no hay administrador. Es una base personal local, con un único perfil configurado; no implementa inicio de sesión multiusuario.

La aplicación escucha solo en `127.0.0.1`. Se rechazan peticiones de otros hosts y de páginas de otros orígenes. Las claves no se envían al navegador: las imágenes se sirven mediante un proxy Rust. No hay analítica ni peticiones a fuentes o servicios externos desde el navegador.

Los avatares locales se leen de `.local/profile-avatars.json`, `MEGALODON_DATA_DIR`, `%LOCALAPPDATA%/Megalodon/app/profile-avatars.json` o los almacenes del proyecto anterior, sin escribir en ellos. Sus imágenes viven en `static/avatars`; el proyecto anterior se admite como alternativa. Se conservan 44 presets y sus equivalencias antiguas. El menú muestra el perfil configurado, no implementa cambio de sesión.

## Desarrollo

La lógica, las rutas, la caché, la búsqueda y las vistas están escritas en Rust. Axum sirve HTML generado con Maud; la presentación usa CSS. `client/src/lib.rs` añade interacción progresiva mediante WebAssembly. JavaScript se limita a `boot.js` y el puente generado por wasm-bindgen: no hay framework, hidratación ni router de cliente.

```powershell
.\dev.ps1                  # ejecutar compilación de desarrollo
.\dev.ps1 -Action test     # pruebas con servidor Jellyfin simulado
.\dev.ps1 -Action lint     # Clippy, sin advertencias permitidas
.\dev.ps1 -Action fmt      # formato Rust
.\dev.ps1 -Action build -Release
.\client.ps1 -Action build # recompilar interacción Rust/Wasm antes del servidor
.\client.ps1 -Action test  # paginación de los carruseles
.\client.ps1 -Action lint
.\tests\smoke.ps1          # comprobar el catálogo real con el servidor abierto
```

En este equipo se ha preparado una toolchain portátil dentro de `.tools`, ignorada por Git. El script la detecta y usa LLVM para enlazar correctamente rutas de Windows con tildes. En otro equipo utiliza Rust estable instalado normalmente: `cargo run --release` (en Windows, con las herramientas de compilación de su toolchain).

Los assets se incluyen al compilar. Después de cambiarlos, recompila, reinicia el servidor y recarga el navegador. `DESIGN.md` explica la dirección visual y las skills utilizadas.

El módulo Wasm y su puente generado se distribuyen en `static/client`; ejecutar la aplicación no requiere compilarlos. Para modificarlo hacen falta el target `wasm32-unknown-unknown` y `wasm-bindgen-cli` 0.2.130. El script `client.ps1` usa las herramientas locales de este equipo. En otras máquinas: `cargo build --manifest-path client/Cargo.toml --target wasm32-unknown-unknown --release`, seguido de `wasm-bindgen client/target/wasm32-unknown-unknown/release/megalodon_ui.wasm --target web --out-dir static/client --out-name megalodon_ui --no-typescript`.

### Archivos

- `src/main.rs`: rutas, servidor local, controles de origen y proxy de imágenes.
- `src/jellyfin.rs`: conexión, modelos, paginación y caché.
- `src/views.rs`: vistas y búsqueda/filtrado.
- `src/avatars.rs`: lectura de preferencias y presets de perfiles.
- `static/app.css`: diseño y adaptación por tamaño de pantalla.
- `static/avatars.css`: encuadres de los avatares originales.

## Verificación

Las pruebas comprueban un catálogo de 1.203 títulos, caché, páginas repetidas o incompletas, caída de Jellyfin, metadatos opcionales, búsqueda con tildes, escape de HTML y paginación de temporadas largas. La prueba real de esta sesión ha conectado con 43 títulos: 37 películas y 6 series.

La biblioteca sigue siendo la misma de Jellyfin. Esta aplicación consulta los datos; no renombra archivos ni cambia metadatos del servidor.
