# Seguridad

## Avisar de una vulnerabilidad

No abras una incidencia pública. Usa **Security → Report a vulnerability** en este repositorio (aviso privado de GitHub) e incluye los pasos para reproducirla.

## Cómo protege Megalodon tus datos

- **Credenciales**: las sesiones de Jellyfin, las contraseñas de WebDAV y las claves personales de proveedores se cifran con DPAPI y solo las puede leer tu cuenta de Windows. Nunca se guardan en el código, en URLs ni en logs.
- **Datos locales**: perfiles, progreso, preferencias y cachés viven en `%LOCALAPPDATA%\MegalodonRust`, fuera del repositorio y de la carpeta de instalación.
- **Red**: el transporte de vídeo de Drive y WebDAV escucha solo en `127.0.0.1`, con credenciales aleatorias por sesión. Las imágenes externas solo se descargan de una lista cerrada de dominios, por HTTPS y sin seguir redirecciones a otros dominios.
- **Escritura**: Megalodon no modifica tus vídeos. Solo escribe en la carpeta de imágenes de tu biblioteca (`posters/`) cuando subes una imagen o un avatar.
- **Actualizaciones**: cada paquete se verifica contra su SHA-256 antes de instalarse.

## Al hacer un fork

- No subas `.local/`, `.env`, `dist/` ni ninguna carpeta de datos; el `.gitignore` ya los excluye.
- Pon tus claves de compilación como *repository secrets*, nunca en archivos.
- Las compilaciones distribuidas con una clave de TMDB incluida permiten extraerla del ejecutable: usa un token de solo lectura.
