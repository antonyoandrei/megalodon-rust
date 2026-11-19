# Megalodon

## Revisión vigente 0.3.6

Ficha a pantalla casi completa con el contenido anclado abajo a la izquierda: logotipo (que nunca invade «Volver»), una línea de año · duración · valoración · géneros, sinopsis de hasta cinco líneas con «Leer más» en un diálogo y sin repetición, progreso «Quedan X min», y acciones grandes. En series, «Continuar T·E» reproduce el episodio en curso o el siguiente al último visto. Seguir viendo añade el siguiente episodio cuando el último visto está terminado. Cinco temas con acento propio en botones, pestaña activa, progreso e interruptores: Cine, Abisal, Ámbar, Nebulosa y Negro puro. Tarjetas con zona activa ajustada a imagen y texto; cargas sin iconos en escenarios grandes.

El progreso visto en un Jellyfin anterior puede importarse a un perfil local; Seguir viendo propone entonces el siguiente episodio.

## Revisión 0.3.5

Episodios: foto propia de TMDB por episodio (una consulta por temporada presente) cuando no hay imagen junto al vídeo. Búsqueda en inglés y, si no coincide, en español. Las elecciones de imagen del usuario se respetan siempre, aunque el proveedor deje de ofrecerlas, y viven en sus datos (no se pierden al actualizar). Sin pestaña Miniatura: en Seguir viendo se extraen en segundo plano cuatro fotogramas reales por título (15–80 % del vídeo, bandas negras recortadas) y cada hover elige uno al azar, que entra con un ease-in estático, sin zoom ni movimiento. Hero hasta 4K, con más altura en pantallas grandes y encuadre hacia el tercio superior.

Primer arranque: bienvenida en cuatro pasos (bienvenida, nombre y color del perfil, tema, biblioteca) que se puede saltar y se completa al conectar una fuente. Quien actualiza desde una versión anterior no lo ve. El instalador ya no ofrece Jellyfin: todas las conexiones, Jellyfin incluido, se añaden desde la app. Desinstalar conserva perfiles, conexiones y preferencias en %LOCALAPPDATA%\MegalodonRust.

Avatares genéricos: se retiran los 44 avatares de personajes del paquete (los del usuario viven ahora en su Drive, posters/avatars, con su encuadre original). Sin foto elegida, color automático distinto por perfil (no se elige en la galería); las imágenes propias viven en la biblioteca (`posters/avatars/<categoría>/` o `avatars/`), cada subcarpeta es una categoría y la galería permite subir con «+» (recorte cuadrado de 512 px). Se decodifican a su tamaño real en pantalla con Lanczos.

Temas en Ajustes → Apariencia: Cine (predeterminado), Abisal, Negro puro y Ámbar; cambian lienzo, superficies, texto y acento, nunca el arte. El icono de la app es el símbolo blanco en todas partes, como pidió el usuario; el instalador refresca la caché de iconos de Windows.

## Revisión 0.3.3

Las tarjetas muestran solo el título (año y tipo fuera; los episodios conservan temporada y capítulo). El destacado vuelve a la composición a todo el ancho: prioriza títulos con fondo panorámico, acepta banners de anime y, si la biblioteca solo tiene carteles, los muestra enteros sobre un campo de color extraído del propio cartel. Las imágenes se piden en píxeles físicos (pantallas al 150 %) y usan mipmaps.

Ajustes deja de crecer con desplegables: conexiones, añadir/quitar conexión, imágenes, reproducción, actualizaciones y más opciones se abren en modales. Añadir conexión ya no dispara la transición de página.

Al conectar una fuente, las imágenes se completan solas, gratis y sin cuentas: el usuario no quiere suscribirse a nada. Orden por defecto: Cinemeta/Metahub (catálogo público de Stremio; carteles, fondos hasta 4K y logotipos de películas y series por IMDb o título exacto y año), TVmaze, AniList, Kitsu y, al final, TMDB y fanart.tv como opcionales con clave. De Cinemeta solo se toman imágenes; su sinopsis inglesa no sustituye texto local. Lista ordenable con interruptor; el primero con una imagen adecuada gana y un fondo 16:9 siempre supera a un banner. Una clave de proyecto puede incluirse al compilar; nunca es obligatoria. Fichas e imágenes en inglés por defecto, sin selector de idioma. Megalodon elige solo la mejor imagen; en la ficha de cada película o serie, el botón de imágenes abre un modal con Cartel, Fondo y Logotipo: «Automático» (recomendada), «Original» de la biblioteca y las alternativas de cada fuente con idioma y resolución. La elección se guarda por título y sobrevive a nuevas consultas.

Imágenes propias: carpeta `posters` (o images, artwork, covers, portadas; nombre configurable) dentro de la biblioteca, una subcarpeta «Título (Año)» con poster, background, logo y thumb. Se detecta sola y gana a la carpeta del vídeo y a internet. El selector de la ficha incluye «Subir imagen»: elige archivo, valida, normaliza (JPG 92 / PNG para logos, máx. 3840 px), crea la carpeta del título y la guarda en la conexión (carpeta local, Drive o WebDAV), y la aplica al momento. Es la única escritura en la fuente y se limita a esa carpeta; Drive creado por Megalodon pide permiso explícito una vez. Las tarjetas horizontales usan thumb cuando existe.

Marca: símbolo, wordmark y logo horizontal blancos entregados por el usuario en static/brand. La navegación usa el símbolo blanco; ventana, barra de tareas e instalador, el mismo símbolo en azul #4C96E8. Ninguna versión abre consola.

## Revisión 0.3.2

Los perfiles pertenecen a Megalodon en todas las conexiones: nombre y avatar, sin contraseña de perfil, favoritos y progreso locales separados por persona y fuente. Jellyfin queda como adaptador opcional; Plex sale del alcance. Drive, WebDAV y carpetas leen directamente archivos y NFO. La instalación de desarrollo se entrega con Drive activo, sin conexión Jellyfin.

El hero rota cada nueve segundos con precarga y fundido, respeta movimiento reducido y cambia al actualizar la fuente. Seguir viendo ocupa la primera fila; se elimina Recién añadidos. El reproductor mantiene botones flotantes sobre una línea de progreso centrada, más corta y ligeramente elevada. Ajustes concentra perfil y conexiones; opciones avanzadas se despliegan. La galería de avatares conserva los originales por colecciones con confirmar y cancelar accesibles.

La cabeza del tiburón sigue el boceto del usuario: mandíbula abierta, hocico orientado a la derecha y silueta triangular, sin play superpuesto. Maestro azul #4C96E8 y variante óptica pequeña.

TMDB y fanart.tv completan opcionalmente fichas e imágenes con autorización propia protegida en Windows. IDs del NFO primero; coincidencias únicas por título y año después. Arte filtrado por tipo, idioma, resolución y proporción. Se conserva el existente salvo opción explícita. No se escribe en la fuente.

## Platform
Aplicación nativa Windows en Rust con eframe/egui y reproducción integrada mediante libmpv/OpenGL. El usuario ha autorizado reproducción e instalador el 17 de noviembre de 2026. Se conserva por separado el prototipo web Axum/Maud y Rust/Wasm.

## Usuario y propósito
Cliente personal que se pueda publicar como código abierto y usar de forma genérica. Cada persona debe conectar sus propias bibliotecas y fuentes desde la aplicación. La instancia de desarrollo usa el Jellyfin del usuario; esa configuración local no define el producto.

Explorar películas y series reales, buscar, filtrar y consultar las fichas, temporadas y episodios completos. La experiencia debe conservar estado y continuidad entre interacciones, como una aplicación.

## Decisiones confirmadas
El usuario ha pedido una carpeta hermana independiente de megalodon-front y lógica en Rust. El código usa Axum y Maud para HTML, CSS para presentación y un módulo Rust/WebAssembly para interacción. JavaScript se limita al puente generado y su cargador.

El 17 de noviembre de 2026 eligió «Cinematográfica e inmersiva, estilo Infuse / Apple TV». Ha rechazado la composición de panel lateral y escenario enmarcado y pide componentes hechos a medida. También ha solicitado recuperar las fotos de perfil, guardadas como preferencias del Megalodon original.

Después ha pedido corregir cortes bruscos en las filas, añadir difuminados, mejorar la adaptación responsive y completar animaciones, transiciones y skeletons reales.

En la revisión 0.2.2 ha descartado expresamente el parallax y los títulos que se mueven con el puntero. El movimiento se limita a carga, transiciones e interacción. Pide mejorar la barra y los menús del reproductor y recuperar el selector de sus avatares originales en el perfil nativo.

En 0.2.3 rechaza la barra contenedora grande y la velocidad: quiere controles flotantes mínimos, play/pausa y saltos de diez segundos grandes sobre el vídeo, como Disney, con iconos bien centrados y aparición temporal al interactuar.

También ha concretado el objetivo de permitir bibliotecas personales y complementos como los de Stremio. La app nativa conecta Jellyfin desde Ajustes, autentica usuarios sin permisos de administrador y almacena la sesión cifrada fuera del repositorio. Stremio, móvil y TV siguen pendientes; no presentar estas integraciones como disponibles hasta implementarlas y comprobarlas.

## Límites
Datos e imágenes reales. No inventar progreso, calidades ni recomendaciones. Conservar búsqueda, filtros, catálogos completos, temporadas y accesibilidad. No modificar el proyecto original ni la configuración del servidor Jellyfin. La app sincroniza progreso como cliente autorizado. Las credenciales permanecen en el proceso Rust y se cifran con DPAPI al guardarse.

En 0.2.4 pide quitar textos repetidos y espacio innecesario, rediseñar temporadas, episodios y perfil, corregir los límites de los carruseles y acumular los saltos rápidos. La navegación interior usa solo el símbolo. El logo debe ser limpio y legible; la última indicación descarta blanco y coral y no obliga a incorporar play si no está integrado con sentido.

La aplicación debe iniciar y cerrar los servicios locales que necesite, reutilizar los procesos existentes sin duplicarlos y ofrecer preparación desde el instalador. Un servidor o montaje que ya estaba abierto no pasa a ser propiedad de Megalodon. La app nativa no necesita ni inicia el servidor web del prototipo.

En 0.2.6 pide transición en cada cambio de pantalla, especialmente avatares; información de temporada y episodio en Seguir viendo; y rehacer el logo con libertad creativa inspirada en el nombre y el animal Megalodon. No obliga a conservar la ola ni a introducir un play.

## MVP 0.3.0

El objetivo inmediato es conectar Google Drive y carpetas locales, con o sin Jellyfin, conservar varias conexiones y poder quitarlas o cambiarlas. Se añaden favoritos por perfil, reanudación precisa y una densidad adecuada en monitores grandes y panorámicos. El cambio de perfil no debe pedir contraseña en cada uso: se verifican las sesiones recordadas y se utiliza Quick Connect autorizado por la cuenta administradora de casa cuando el servidor lo permite. No se desactiva la autenticación del servidor.

El instalador debe sustituir versiones anteriores conservando datos e incorporar avisos y actualización desde las publicaciones del propio proyecto. El nombre visible sigue siendo Megalodon.

El usuario ha pedido instalar y aplicar `kaankiziltug/logo-design-skill`. Tras rechazar las primeras propuestas, ha delegado la elección: «decide lo mejor, ten en cuenta que se llama megalodon». Se aplica Marea: cabeza de tiburón azul con mandíbula abierta y la ola integrada en su silueta. No lleva play ni wordmark en la navegación. Se conservan los 44 avatares originales. Las pruebas con receptores físicos siguen aplazadas por petición expresa.

TVmaze completa imágenes de series por defecto, sin clave, con coincidencias únicas por ID o título y año. Se puede desactivar en Ajustes → Imágenes y fichas. No reemplaza las sinopsis españolas con texto en otro idioma. [API y atribución CC BY-SA](https://www.tvmaze.com/api). TMDB y fanart.tv requieren autorización propia.
