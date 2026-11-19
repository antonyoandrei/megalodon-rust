# Conexiones, perfiles e imágenes

Megalodon lee vídeos sin necesitar Jellyfin. Engranaje → Añadir conexión ofrece Carpeta local, Google Drive, Mi nube (WebDAV) o Jellyfin. Puedes guardar varias, abrir otra o quitarlas sin borrar archivos. La búsqueda y el catálogo pertenecen a la conexión activa.

El avatar abre los perfiles y «+» crea uno con nombre e imagen, sin contraseña. Favoritos, progreso y ajustes de experiencia pertenecen a cada perfil local, incluso usando Jellyfin. La app no escribe el progreso en cuentas del servidor. Las sesiones recordadas anteriores se importan conservando imágenes e historial ya guardado en el equipo. No se sincronizan perfiles a Drive ni entre ordenadores.

Las autorizaciones de Google, WebDAV o Jellyfin permiten leer la biblioteca; no son contraseñas de perfil. Las credenciales se protegen para la cuenta de Windows y no se exportan con el código. Drive usa autorización de lectura y rclone incluido. El transporte HTTP autenticado escucha en localhost y se cierra al cambiar de conexión o salir. Los servicios abiertos manualmente no se cierran.

Los NFO locales y remotos aportan títulos, sinopsis, fechas, temporadas, episodios e IDs. Se conservan las imágenes de las carpetas. Los NFO remotos se copian a una caché local sin modificar la fuente ni descargar los vídeos completos.

## Completar fichas y arte

Ajustes → Imágenes y fichas permite añadir un token de lectura de TMDB y una clave personal de fanart.tv. Son opcionales; no se incluyen claves de terceros. Los enlaces llevan a cada proveedor. Las autorizaciones se guardan cifradas con DPAPI y pueden desconectarse.

Los IDs TMDB/IMDb/TVDB del NFO prevalecen. Sin IDs se exige coincidencia única por título y año cuando existe. Películas y series reciben ficha en español, carteles verticales, fondos panorámicos preferentemente sin texto y logotipos PNG. Se filtran miniaturas, proporciones incorrectas e idiomas; se compara resolución útil y valoración. Las coincidencias dudosas conservan el original. Los capítulos mantienen los datos NFO.

Solo se rellenan imágenes ausentes por defecto. «Mejorar también las imágenes existentes» permite reemplazarlas dentro de la app. Los archivos permanecen intactos. Consultas en caché siete días; arte descargado con caché local. No se envían perfiles, favoritos ni posiciones a los proveedores.

Fuentes: [TMDB e idiomas](https://developer.themoviedb.org/docs/image-languages), [atribución TMDB](https://developer.themoviedb.org/docs/faq), [API fanart.tv](https://api.fanart.tv/), [WebDAV](https://rclone.org/webdav/).

El envío directo de Drive/carpetas/WebDAV a receptores sigue pendiente; los adaptadores de transmisión existentes usan Jellyfin. Las pruebas de hardware permanecen aplazadas.

TVmaze completa imágenes de series por defecto, sin clave, con coincidencias únicas por ID o título y año. Se puede desactivar en Ajustes → Imágenes y fichas. No reemplaza las sinopsis españolas con texto en otro idioma. [API y atribución CC BY-SA](https://www.tvmaze.com/api). TMDB y fanart.tv requieren autorización propia.

Los nombres alternativos se aceptan solo cuando el proveedor los enumera expresamente y coincide el año conocido. Los episodios sin miniatura propia utilizan el fondo de su serie como respaldo visual, conservando su número y título.
