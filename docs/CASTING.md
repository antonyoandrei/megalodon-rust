# Transmisión

El botón de pantalla del reproductor busca receptores solo cuando se abre. La app no busca ni controla televisores al arrancar.

| Destino | Implementación | Estado de comprobación |
| --- | --- | --- |
| Chromecast / Google Cast con vídeo | Descubrimiento mDNS, Cast V2, receptor multimedia, pausa, reanudación, saltos, posición y fin de sesión | Código nativo y pruebas de protocolo; pendiente de receptor físico |
| LG y otros MediaRenderer DLNA | SSDP, descripción XML, AVTransport SOAP, Play/Pause/Seek/Stop y posición | Código nativo y pruebas de protocolo; pendiente de modelos físicos |
| AirPlay | Enlace temporal abierto en Safari de iPhone/iPad/Mac; controles AirPlay propios del sistema y progreso comunicado por Rust/Wasm | Sin pruebas físicas; requiere Safari y un receptor compatible |

AirPlay no es un emisor AirPlay 2 nativo para Windows. El cliente anterior también utilizaba las funciones de Safari. No se presenta el antiguo HTTP `/play` como compatible con todos los Apple TV modernos.

## Vídeo y sesión

Jellyfin negocia MP4/H.264/AAC para reproducción directa o HLS/H.264/AAC para transcodificar. Esto requiere que la cuenta y el servidor permitan la conversión cuando el archivo original no sea compatible. Algunos televisores DLNA no admiten HLS o búsquedas en determinados formatos: la aplicación muestra el rechazo del receptor. HDR, pistas externas y todas las combinaciones de códecs todavía no tienen verificación de hardware.

Un relay Rust temporal sirve exclusivamente el título elegido. Los receptores reciben una URL aleatoria de 192 bits, nunca la contraseña, la API key ni el token de Jellyfin. Chromecast/DLNA restringen el relay al IP del receptor elegido. El enlace de Safari permite dispositivos de la misma red mediante el ticket temporal. Se limita el tamaño de mensajes y listas, se respetan peticiones Range y se reescriben los segmentos HLS sin credenciales. No se siguen redirecciones del servidor con autenticación ni se admiten URLs ajenas a ese título.

El cierre de la transmisión invalida el enlace y corta las respuestas de vídeo activas. Los controles comprueban el identificador o la URL de la sesión antes de enviar órdenes. Cambiar de perfil o cerrar Megalodon termina la sesión que inició la app. La posición recibida se informa usando la sesión del perfil que la abrió.

No se abre ningún puerto permanente ni se cambia el firewall. Windows puede pedir acceso a la red privada cuando se solicita transmitir. Los dispositivos deben poder comunicarse entre sí; las redes de invitados con aislamiento no sirven para este flujo.

Referencias: [Cast V2 en rust-cast](https://github.com/azasypkin/rust-cast), [formatos de Google Cast](https://developers.google.com/cast/docs/media), [Jellyfin MediaInfo API](https://typescript-sdk.jellyfin.org/functions/generated-client.MediaInfoApiFactory.html), [limitaciones del flujo AirPlay heredado](https://docs.rs/cidermill-airplay/latest/cidermill_airplay/play/index.html).
