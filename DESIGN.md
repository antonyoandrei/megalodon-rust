# Megalodon · dirección cinematográfica

## Contexto y decisiones del usuario

El usuario ha rechazado las dos primeras versiones y ha elegido explícitamente: «Cinematográfica e inmersiva, estilo Infuse / Apple TV». Pide componentes propios y recuperar las fotos de perfil. La barra lateral azul, las métricas decorativas, el escenario dentro de una tarjeta y el texto de marketing son antirreferencias, no elementos que conservar.

## Escena y propósito

Elegir una película o una serie de una biblioteca personal. La escena prevista es una pantalla para disfrutar del contenido: arte primero, navegación tranquila, información útil al acercarse. Se conserva el catálogo real y todas sus funciones. El negro del cine y los colores de cada película ocupan la composición; el azul del tiburón queda en la marca, el foco y los estados de interacción.

Dominio: fotograma, cartel, colección, temporada, reparto, sinopsis. Mundo de color: sala oscura, subtítulos blancos, créditos grises, azul de la marca y colores originales del arte. Firma: un lienzo continuo con el logotipo real de la obra, selección manual de fotogramas y estanterías que alternan descubrimiento horizontal y colección vertical.

## Primera pantalla y recorrido

- Navegación superior sobre la imagen. Cuatro destinos de texto, búsqueda y avatar real del perfil.
- Fotograma a todo el ancho, sin marco, con degradados funcionales para legibilidad y continuidad hacia el fondo.
- Logotipo original, metadatos reales, dos líneas de sinopsis y una acción «Explorar película/serie».
- Selector manual de cuatro destacados: miniaturas en escritorio e indicadores con área táctil en móvil. Sin cambio automático.
- Recién añadidos con imágenes horizontales, películas con carteles 2:3 y series con fotogramas. Sin recomendaciones o progreso inventados.
- La ficha mantiene la composición cinematográfica y lleva a información, temporadas y reparto mediante enlaces reales.

## Componentes propios

Implementados en funciones Maud y CSS, sin kits visuales. La interacción progresiva se escribe en Rust y compila a WebAssembly; un cargador JavaScript mínimo y el puente generado por wasm-bindgen la conectan con el navegador. Los enlaces, formularios y menús conservan el comportamiento HTML nativo.

- Navegación: 96 px de alto en escritorio; barra inferior de cuatro destinos en móvil. Un único destino activo.
- Perfil: imagen de 42 px, 34 px en móvil, menú con identidad y enlaces reales. Se respeta el recorte y la elección del Megalodon original.
- Buscador: panel nativo `popover`, foco al abrir, Enter para buscar, Escape para cerrar.
- Filtros: botones de 46 px y popovers dibujados a medida; opciones como enlaces que conservan búsqueda, tipo, género y orden, reiniciando página.
- Tarjeta de colección: cartel 2:3, radio 12 px, título y metadatos debajo. El visto procede de Jellyfin.
- Tarjeta de descubrimiento: 16:9, radio 12 px, título debajo; foco y elevación sutil al interactuar.
- Selector de temporada: popover con temporada actual siempre en el botón. Las temporadas largas se paginan a 40 episodios.
- Episodio: fotograma, número, título, duración y sinopsis nativa desplegable.
- Reparto: retratos circulares y nombres reales.

## Tokens y tipografía

| Token | Valor | Uso |
| --- | --- | --- |
| cinema | #08090b | Lienzo |
| screen | #131518 | Controles y huecos de imagen |
| menu | #202226 | Menús superpuestos |
| light | #f5f5f7 | Texto principal y acción |
| subtitles | #c5c7cc | Texto secundario |
| credits | #a0a4ad | Metadatos |
| edge | #ffffff1f | Separación funcional |
| focus | #86c9fa | Foco visible |

La interfaz usa la fuente de la plataforma (SF en Apple, Segoe UI Variable en Windows), por la dirección de aplicación elegida. La voz visual de los títulos viene de los logotipos reales. No se descargan fuentes ni se imita una landing editorial. Escala: cuerpo 16 px; controles 14–15; metadatos 12–14; secciones 26; catálogo 40. En móvil, cuerpo 14–15, secciones 23, catálogo 34. Tracking mínimo -0.035em.

Base espacial 4 px; separación entre controles 8–12; tarjetas 16–24; secciones 40–56. Gutter adaptable 24–96 px. Los popovers tienen una única elevación con sombra; el resto se separa por espacio.

## Continuidad y carga

El usuario ha señalado cortes bruscos en los extremos, adaptación incompleta y falta de transiciones y skeletons. Las filas ahora reservan tarjetas completas en su área útil, extienden el desplazamiento hasta el borde del lienzo y difuminan solo los márgenes cuando queda contenido en esa dirección. Flechas de 44 px, teclado y desplazamiento nativo; en móvil, una tarjeta horizontal o dos carteles completos por vista.

El hero cambia dentro de la página: conserva el fondo actual hasta decodificar el siguiente y funde el nuevo encima durante 420 ms, manteniendo una base opaca. La acción conserva brillo y posición. El texto anterior sale en 100 ms y el nuevo entra después, sin superponer sinopsis. La última selección gobierna los cambios rápidos; un fallo conserva la escena actual y permite reintentar. La navegación entre páginas dura 220 ms. Menús con entrada y salida de 160–220 ms; sinopsis con expansión de 240 ms. No hay coreografía al hacer scroll ni cambios automáticos de destacado.

Cada imagen reserva su relación de aspecto. El skeleton existe únicamente durante su carga real, anima solo dentro del viewport y se detiene al cargar, fallar u ocultarse la pestaña. Las imágenes en caché no esperan. La entrada de la imagen dura 280 ms. Los errores dejan una alternativa visible; los títulos siguen siendo accesibles. Mientras navega una petición se muestra un indicador indeterminado, nunca un porcentaje inventado.

Con movimiento reducido, desplazamiento inmediato, skeleton estático y entrada de imagen de 100 ms; las transiciones de navegación quedan desactivadas. El módulo no intercepta rutas ni sustituye el renderizado de Rust. Sin WebAssembly siguen funcionando el catálogo, los enlaces, la búsqueda y el desplazamiento nativo.

## Verificación

Revisar el viewport real del usuario (1280×720), 390×844 y tableta. Probar búsqueda, filtros combinados, menús abiertos/cerrados, Escape, foco, estados vacíos, texto largo, fichas y temporadas. Comprobar las fotos contra los presets originales; nunca reemplazar una elección guardada por una inicial.

## Skills de referencia

[Interface Design](https://github.com/Dammyjay93/interface-design): jerarquía y componentes de producto.
[Impeccable](https://github.com/pbakaus/impeccable): revisión del resultado renderizado y eliminación de patrones genéricos.
[Web Design Guidelines](https://github.com/vercel-labs/agent-skills/tree/main/skills/web-design-guidelines): interacción y accesibilidad.

Estas skills guían el trabajo. La dirección confirmada por el usuario, los datos reales y la base Rust tienen prioridad sobre sus recetas generales.

## Aplicación nativa · revisión 0.2.1

El usuario ha pedido menos botones, menús flotantes y una profundidad ligera en los títulos. El reparto se retira de las fichas nativas. La referencia visual sigue siendo la biblioteca cinematográfica de Infuse / Apple TV; no se añaden paneles decorativos ni texto promocional.

- Controles e iconos vectoriales en Rust: un único sistema de trazo, foco visible, cursor de mano, respuesta de pulsación de 100 ms y hover de 160 ms. Las áreas interactivas no se desplazan.
- Título en una capa con desplazamiento máximo de 6 px y perspectiva muy leve; fondo en una capa de movimiento contrario de 4 px. Sin oscilación automática ni elementos que persigan al puntero. El texto, los botones y sus hitboxes permanecen estables.
- Navegación con la composición anterior sobre la nueva durante 200 ms; el fundido se puede interrumpir conservando la composición visible. El header se mantiene. Las texturas de la escena anterior permanecen vivas hasta terminar.
- Destacados sin cambio automático: la nueva imagen y el logotipo se cargan antes del fundido de 420 ms. La última selección queda en espera mientras termina la transición actual. Imágenes de tarjetas con entrada de 240 ms.
- Skeletons de hero, tarjetas y episodios mientras se esperan datos reales; pulso común discreto que se detiene fuera del viewport, sin foco o al terminar. La reproducción tiene una espera cancelable y nunca reutiliza el fondo de la biblioteca.
- Reproductor: transporte centrado, volumen a la izquierda, ajustes y pantalla completa a la derecha. Barra con posición y tramo almacenado reales, tiempo bajo el puntero y búsqueda al soltar. Audio, subtítulos y velocidad comparten un menú flotante con navegación de teclado. Los subtítulos reservan margen para los controles visibles y recuperan su posición al ocultarlos.
- Menús con superficie #1c1e23, radio 16 px y una sombra suave desplazada. Filas de 44 px, selección visible, cierre exterior y Escape. Las preferencias usan interruptores propios.
- Movimiento reducido: perspectiva y desplazamientos desactivados, skeletons estáticos, navegación inmediata y entrada de imágenes de 80 ms. Se preservan las señales de foco, selección y carga.

## Aplicación nativa · revisión 0.2.2

La última indicación del usuario sustituye la profundidad de 0.2.1: no hay parallax, perspectiva ni movimiento de títulos con el puntero, tampoco en las fichas. La imagen y el título permanecen quietos.

El momento principal es el cambio manual de destacado. Las cuatro escenas se precargan con sus logotipos; el fondo anterior permanece opaco bajo el fundido de 340 ms. El texto sale antes de entrar el nuevo y las acciones apuntan al título visible. La primera carga coordina fondo y título en 240 ms, sin repetir su entrada al volver a Inicio. El movimiento restante confirma carga, navegación e interacción. Los clics rápidos tienen una respuesta de 75 ms, con área de interacción fija. No hay animaciones de fondo en reposo.

El reproductor reúne transporte, tiempo, volumen y opciones en una sola barra flotante. El menú tiene pestañas Audio, Subtítulos y Velocidad, ancho estable y altura ajustada a su contenido, pistas con nombre y metadatos reales, selección persistente visible y cierre exterior o con Escape. La altura se adapta en 180 ms; su entrada dura 180 ms y la salida 120 ms; el contenido de cada pestaña entra en 160 ms. Las pistas se consultan al abrir y al cambiar selección, no en cada frame.

El perfil permite elegir entre los 44 avatares originales, agrupados por colección, con los nombres, zoom y posiciones recuperados del proyecto original. La galería se despliega dentro de la página; mantiene el avatar seleccionado y guarda su elección por servidor y usuario. Las imágenes se decodifican fuera del hilo de interfaz y se conservan en la caché existente. La elección local tiene prioridad sobre el avatar heredado y una respuesta de red antigua no puede sustituirla.

Con movimiento reducido se conservan selección, foco y respuesta visual; el hero cambia inmediatamente y las entradas de imágenes duran 80 ms. Verificación: 1280×800 y 640 px, selección persistente de avatar, reproducción real 4:3, teclado, búsqueda, apertura/cierre del menú y cambios de pistas.

## Aplicación nativa · revisión 0.2.3

El usuario ha rechazado la barra contenedora de 0.2.2: pide botones flotantes que ocupen menos y elimina el selector de velocidad. Play/pausa y los saltos de diez segundos pasan al centro del vídeo, en controles circulares de 80 y 68 px. Los iconos de salto son geometría vectorial propia, con el número 10 centrado independientemente de métricas de fuente. Los controles aparecen al interactuar y se desvanecen tras 2,8 segundos de inactividad, también en pausa; menú, teclado y búsqueda mantienen visibles los controles mientras se utilizan.

Abajo quedan una línea de progreso fina, tiempo en una sola línea y dos accesos circulares de 44 px para Audio y subtítulos y pantalla completa. No hay panel que englobe la parte baja. El volumen está en la pestaña Audio. El menú se reduce a 320 px de ancho, dos pestañas, cierre y las pistas reales, sin cabecera redundante. Conserva foco, selección, animación de altura y cierre exterior/Escape. Se mantienen los cambios del hero y los avatares de 0.2.2.

## Aplicación nativa · revisión 0.2.4

Se eliminan los contadores verbosos y la repetición de «en tu biblioteca». Episodios muestra solo el número de la temporada activa, centrado en la misma fila que el título. Las temporadas pasan a pestañas T1, T2… y Especiales, con ajuste de línea, teclado y selección clara. Los episodios son una cuadrícula adaptable de fotogramas 16:9, título, duración real y sinopsis breve. Se pagina a 24 solo cuando hace falta.

El perfil reduce su cabecera y ofrece categorías de avatar que se ajustan al ancho. Seleccionar guarda y cierra la galería. Las celdas ya no avanzan el cursor de layout después de haber reservado la cuadrícula: se elimina el hueco acumulado antes de los ajustes. Se mantienen los 44 originales y sus recortes.

El carrusel desactiva sus flechas en los límites y acota el destino antes de interpolar; repetir clic al final no inicia un desplazamiento fuera de rango. Los saltos del reproductor se acumulan durante una ráfaga de 900 ms, muestran +20 s / +30 s y se envían a mpv tras 140 ms sin pulsaciones; siempre parten del destino pendiente. Se retiran los tooltips de los botones, conservando nombres accesibles. El panel de pistas mantiene posición y altura entre Audio y Subtítulos; el contenido cambia sin desaparecer y el indicador de pestaña se desplaza suavemente.

La barra de navegación muestra únicamente el símbolo, centrado en 48 px, sin el wordmark. El icono nativo se genera desde static/brand/mark.svg con fondo transparente y tamaños específicos de 16 a 256 px, sin un cuadrado detrás. El tiburón y la ola priman sobre un play añadido artificialmente.

## Aplicación nativa · revisión 0.3.5

Seguir viendo: la tarjeta 16:9 muestra el arte del título o la foto del episodio; al pasar el ratón o con foco, un fotograma aleatorio del vídeo entra en 380 ms con opacidad ease-in (t²) y sale igual. Imagen estática: sin zoom, desplazamiento ni parallax. Hero con altura hasta ancho/2,4 y recorte centrado en el tercio superior.

## Aplicación nativa · revisión 0.3.4

Paleta por tema (controls::THEMES): cinema, screen, text, muted, focus y menu. Cine #08090b/#86c9fa; Abisal #050b16/#60baff; Negro puro #000000/#86c9fa; Ámbar #0e0b09/#ffba66. Tarjetas de tema con miniatura de la interfaz en sus propios colores, 2 columnas, borde de foco y check.

Bienvenida: columna de 680 px, barra de progreso de 4 segmentos, títulos de 30–34 px, acción principal abajo a la derecha y «Atrás» a la izquierda. Fuentes de biblioteca como filas de 72 px con icono, título, detalle y flecha.

Avatares: círculo de color con inicial blanca al 42 % del diámetro; imágenes en cover centrado. Galería con pestañas por categoría que se reparten en filas y casilla «+» al final.

## Aplicación nativa · revisión 0.3.3

Tarjeta: arte y título; sin año ni tipo. Destacado a todo el ancho con arte panorámico (fondos 16:9, thumbs o banners de 2:1 a 5:1); carteles íntegros con sombra sobre un campo de color de 12×18 muestras del propio cartel, sin cuadros centrados ni ampliaciones. Título sin logotipo apoyado sobre la línea de metadatos, 44 px y 34 px si ocupa dos líneas.

Diálogos: superficie de menú, margen 28×24, título 22 px, cierre arriba a la derecha, fondo atenuado al 59 %, Escape y clic exterior. La acción principal va abajo a la derecha. Selector de tipo de conexión segmentado de cuatro partes iguales. Ajustes muestra filas de 72 px con título, resumen y flecha. Proveedores: filas de 68 px con orden, nombre, estado, subir/bajar e interruptor.

Selector de imágenes: modal de 820 px con pestañas Cartel / Fondo / Logotipo; rejilla de 5 carteles 2:3, 3 fondos 16:9 o 3 logotipos sobre gris #262930; título y fuente · idioma · resolución bajo cada miniatura; selección con borde de foco y check. La primera opción es siempre «Automático»; la segunda, en bibliotecas de archivos, «Subir imagen» con un + sobre círculo translúcido y borde suave, que muestra carga mientras guarda. Pestaña Miniatura (16:9) para thumbs. En Imágenes y fichas, una tarjeta «Tus imágenes» con estado de la carpeta y su estructura precede a las fuentes en internet.

## Aplicación nativa · revisión 0.2.6

La última petición vuelve a centrar la marca en el animal y el nombre Megalodon. Se sustituye el tiburón con ola separada por una única silueta azul #559DE8: cabeza ancha, mandíbula abierta, dientes y aletas. Sin play, palabra, contenedor, degradado ni ilustración tridimensional. El SVG es la fuente de los PNG e ICO transparentes.

La navegación comparte un fundido de 240 ms, incluida la apertura y salida de avatares, el formulario de acceso o creación de perfil y las fichas de títulos distintos. La cabecera conserva su posición. Un segundo cambio parte de la composición visible; Escape se resuelve antes de capturar el destino para evitar un fotograma de salto. Cada pantalla interior conserva su propio desplazamiento. La reproducción entra y sale sobre negro opaco y nunca reutiliza el callback de vídeo como una imagen congelada. Movimiento reducido elimina el fundido sin ocultar carga, foco ni selección.

«Seguir viendo» muestra el nombre de la serie y, debajo, temporada, episodio y título del capítulo, con dos líneas disponibles y el progreso real sobre el fotograma. Temporada cero se presenta como Especiales; los números ausentes no se inventan. El nombre accesible contiene el mismo contexto.
