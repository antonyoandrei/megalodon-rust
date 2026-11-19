# Megalodon 0.3.2 · Windows

Abre `Megalodon.exe` o instala con `Megalodon-0.3.2-windows-x64-setup.exe`. No necesitas Rust, Node, Electron ni VLC para el cliente nativo. Requiere Windows 10/11 x64 y un controlador gráfico con OpenGL. Google Drive abre el navegador solo para autorizar la cuenta. Después de un pull, `Instalar.cmd` obtiene el paquete del commit cuando el flujo de GitHub haya terminado; consulta [Distribución](DISTRIBUTION.md).

El instalador instala para tu usuario en `%LOCALAPPDATA%/Programs/Megalodon`, crea accesos en Inicio y Escritorio y registra la desinstalación en Windows. Sustituye la instalación anterior conservando sus datos. No requiere administrador para el cliente. Los archivos personales permanecen en `%LOCALAPPDATA%/MegalodonRust`. Puedes copiar la versión portable completa a otra carpeta.

## Conectar una biblioteca

Pulsa el engranaje junto al avatar → Añadir conexión. Puedes conservar varias conexiones y abrir cualquiera de ellas. Quitar una conexión no borra sus películas ni la carpeta original.

- **Google Drive:** autoriza tu cuenta en el navegador o utiliza una configuración existente de rclone. La app incluye rclone y sirve los vídeos solo en localhost, con autenticación. No necesita Jellyfin ni montar una unidad. La autorización nueva solicita lectura; el archivo de credenciales queda en la carpeta de datos del equipo.
- **Carpeta local:** selecciona una carpeta. Se leen sus vídeos, imágenes y metadatos NFO existentes sin modificarlos. Los nombres `Serie.S01E02.mkv` y las carpetas `Season 01` permiten organizar temporadas y episodios.
- **WebDAV:** introduce la dirección de la carpeta en tu NAS o nube y las credenciales que requiera. Se comprueba el acceso antes de guardarlo; la contraseña se protege con DPAPI. No requiere Jellyfin. [Conexiones y perfiles](CONNECTIONS.md) detalla las diferencias entre modos.
- **Jellyfin:** introduce dirección, usuario y contraseña. La sesión se cifra con DPAPI, vinculada a tu usuario de Windows; no guardamos la contraseña. No necesitas una clave de administrador para ver tu biblioteca. Se importa la conexión del Megalodon anterior cuando todavía no hay conexiones guardadas.

Inicio muestra destacados, novedades y Seguir viendo. Películas y Series filtran y ordenan el catálogo; la lupa o Ctrl+F buscan ambos tipos. Favoritos reúne Seguir viendo y las películas y series guardadas con el corazón. En una serie, selecciona temporada y episodio. Seguir viendo identifica temporada, número y título del capítulo.

Todos los perfiles pertenecen a Megalodon: nombre, avatar, favoritos y progreso locales, sin contraseña para crearlos o cambiarlos. Las credenciales de Jellyfin solo autorizan la conexión a la biblioteca. Las personas recordadas se importan y el historial guardado se vincula a los archivos directos únicamente cuando la coincidencia es única y exacta. No se sincronizan los perfiles con la nube.

El avatar abre los perfiles y «+» crea uno con nombre e imagen. Ajustes → Personalizar muestra los 44 avatares originales por colecciones. Guardar confirma; Cancelar conserva la anterior. Imagen, volumen, movimiento y siguiente episodio son por persona; conexiones y actualizaciones pertenecen a la instalación.

## Reproducir

- Pulsa Reproducir. Si hay progreso, puedes reanudar o empezar de nuevo en la ficha.
- Archivos locales: arrastra el vídeo a la ventana, usa la ruta en Ajustes o ejecuta `Megalodon.exe --open "C:\Videos\pelicula.mkv"`.
- Espacio pausa. Izquierda y derecha saltan diez segundos. M silencia. F activa pantalla completa. Escape sale de pantalla completa o vuelve a la biblioteca.
- Los controles grandes de reproducción y salto flotan sobre el centro del vídeo y desaparecen al dejar de interactuar. Una línea fina permite buscar una posición y muestra el tiempo bajo el puntero. El botón de Audio y subtítulos abre un panel estable de dos pestañas; el volumen está dentro de Audio. Los saltos rápidos se acumulan (+20 s, +30 s…) antes de buscar el destino definitivo. No hay selector de velocidad. Izquierda y derecha cambian de pestaña dentro del panel; arriba, abajo y Enter recorren y seleccionan pistas. Escape cierra primero el menú. Los atajos de Windows no activan la reproducción.
- El progreso se guarda localmente cada diez segundos y al salir. Un episodio terminado sale de Seguir viendo; puede encadenarse el siguiente desde Ajustes.
- Se respeta el formato original, incluido 4:3, con bandas negras opacas. El arte de la biblioteca no se dibuja detrás del vídeo.

## Interfaz y movimiento

El fondo y los títulos permanecen fijos, sin parallax. El hero precarga sus escenas y logotipos, mantiene la imagen anterior durante el fundido y conserva las acciones en su posición. Ajustes → Reducir movimiento deja los cambios inmediatos y los skeletons estáticos. Las cargas conservan espacio; los controles responden al pulsarlos y los paneles entran y salen con un fundido breve. Ctrl+F abre y enfoca la búsqueda. El reparto se ha retirado de las fichas.

## Ver en otro dispositivo

Al reproducir desde Jellyfin, el botón de pantalla permite buscar Chromecast y televisores DLNA, incluidos modelos LG compatibles. AirPlay se ofrece mediante un enlace temporal que se abre en Safari en iPhone, iPad o Mac. No es un emisor AirPlay 2 para Windows. El envío directo desde Drive y carpetas está pendiente. La transmisión tiene pruebas locales de protocolo; los receptores físicos se comprobarán más adelante por decisión del usuario. Consulta [Transmisión](CASTING.md) para formatos y límites.

## Actualizar

Las compilaciones publicadas incorporan el repositorio de origen y consultan sus versiones públicas. El aviso permite descargar el setup, verificar su SHA-256 y reiniciar para actualizar. Ajustes muestra el estado y permite buscar actualizaciones manualmente. El setup espera al cierre de la app y de sus servicios antes de sustituir los archivos. Una compilación local sin repositorio configurado lo indica; no tiene un canal de actualizaciones ficticio.

## Alcance

La interfaz y lógica están escritas en Rust. La reproducción usa libmpv/FFmpeg, bibliotecas nativas con código en C y otros lenguajes. Los códecs no se han reescrito en Rust.

Esta versión de escritorio es Windows x64. Móvil, TV, HDR de extremo a extremo y complementos Stremio todavía no están implementados. La compilación local no está firmada digitalmente. El prototipo web se conserva por separado.

## Resolver problemas

Comprueba que Jellyfin y la unidad de los vídeos están disponibles. Ajustes permite revisar la conexión y actualizar el catálogo. Conserva `libmpv-2.dll` junto al ejecutable. Las imágenes se guardan en una caché local; las credenciales no se escriben en las URLs del vídeo.

Desinstala desde Configuración de Windows o ejecuta `Uninstall.exe --uninstall`. Se conserva la configuración personal y no se toca el servidor ni tus películas.

## Servicios locales

El cliente nativo no requiere Node, Electron ni el servidor web de desarrollo. El instalador incluye libmpv y el runtime gráfico necesario. La opción «Preparar también Jellyfin en este equipo» reutiliza los componentes existentes y obtiene los que falten mediante el catálogo oficial de winget. También está disponible en Ajustes → Preparar servicios. Las fuentes oficiales documentan [Jellyfin para Windows](https://jellyfin.org/docs/general/installation/windows/) y [rclone con WinFsp](https://rclone.org/commands/rclone_mount/).

La instalación del cliente es por usuario; instalar Jellyfin o WinFsp por primera vez puede requerir permisos de administrador de Windows. Una carpeta vacía no se considera un montaje preparado: el arranque espera a que WinFsp presente la unidad.

Si existe una instalación del Megalodon anterior, se leen sus rutas de Jellyfin y Drive sin modificar su proyecto ni copiar credenciales al paquete. La configuración local queda en services.json dentro de la carpeta de datos. Los servidores remotos se conectan directamente. La gestión automática local se aplica a localhost:8096.

Al abrir la app se comprueban primero el puerto, los procesos y el montaje; los servicios existentes se reutilizan y permanecen abiertos al salir. Cuando Megalodon inicia Jellyfin y Drive, conserva sus handles y solicita el cierre de Jellyfin antes de desmontar la unidad. El cierre del servidor requiere una cuenta administradora de Jellyfin. Si el servidor rechaza el cierre o no termina, se conserva el montaje para proteger sus operaciones y se registra el motivo en services-shutdown.txt. No se termina ningún proceso por su nombre.

Solo puede abrirse una instancia nativa por carpeta de datos. Una segunda apertura lleva a la ventana existente; --open remite el archivo a esa instancia. En una instalación local nueva, completa la configuración inicial de Jellyfin y conecta tu cuenta desde la app.
