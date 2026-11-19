# Distribuir Megalodon

Esta carpeta es el proyecto nativo independiente. No necesita archivos del proyecto hermano para compilar o instalar. Las rutas y credenciales personales se guardan fuera del repositorio. La importación del cliente anterior es opcional y solo ocurre en el equipo que ya lo tenía.

## Push, pull e instalación

1. Sube el contenido de esta carpeta a tu repositorio de GitHub y habilita Actions. El flujo `.github/workflows/windows.yml` compila, ejecuta las pruebas y produce instalador, portable, fuentes y hashes.
2. Un push a la rama predeterminada publica una versión preliminar `build-<SHA completo>` una vez superados los checks. Los PR y otras ramas generan artefactos, sin publicar versiones.
3. En el portátil: `git pull` y doble clic en `Instalar.cmd`. El script busca exactamente la publicación del commit descargado; no instala silenciosamente una versión anterior. También puedes descargar el setup desde Releases.
4. El instalador incluye libmpv, sus bibliotecas, rclone y los avatares. Añade una carpeta local, conecta Google Drive desde el navegador o entra en tu servidor Jellyfin. Carpetas y Drive no necesitan un servidor ni WinFsp. Si quieres un Jellyfin local nuevo, marca su preparación en el instalador y completa el asistente inicial del servidor con tus carpetas.

No se transfieren sesiones DPAPI entre equipos: la cuenta se conecta una vez en cada instalación. El equipo receptor no necesita compilar Rust. La ejecución de GitHub Actions y la instalación en un segundo equipo no se han probado desde esta sesión local.

Un repositorio privado exige autenticarse también para descargar sus Releases: el script admite `GH_TOKEN` o el acceso existente de `gh`. Nunca escribe ese token en el proyecto. Si la carpeta no tiene un remoto GitHub, se puede indicar `./Instalar.ps1 -Repository propietario/repositorio` dentro del checkout correspondiente.

## Actualización desde 0.2.4

El nombre visible es Megalodon. El instalador migra su paquete anterior de `Programs/Megalodon Rust` a `Programs/Megalodon`, actualiza Inicio, Escritorio y la entrada de desinstalación. La carpeta de datos conserva su nombre interno `MegalodonRust` para no perder preferencias ni sesiones. No migra ni elimina instalaciones ajenas sin el registro propio de archivos.

## Actualizaciones dentro de la app

Actions incorpora `MEGALODON_UPDATE_REPO` y `MEGALODON_BUILD_COMMIT` al compilar. Megalodon consulta las publicaciones públicas de ese repositorio al abrirse, muestra un aviso discreto y permite descargar el setup. Comprueba la suma SHA-256 exacta antes de ofrecer «Reiniciar y actualizar». El setup espera el cierre normal de la app, actualiza su paquete y vuelve a abrirla. El instalador normal también busca una publicación posterior antes de instalar; sin conexión conserva su paquete incluido.

La descarga solo admite GitHub y sus hosts de assets por HTTPS. Una suma SHA-256 verifica integridad; no sustituye la firma digital del ejecutable. El canal automático actual usa Releases públicas: los repositorios privados pueden instalarse con `Instalar.cmd`, pero el cliente no reutiliza `GH_TOKEN`. Una compilación local sin repositorio asociado lo indica en Ajustes. Esta carpeta aún necesita un remoto y una publicación propia para probar el aviso real de una versión posterior.

## Compilar localmente

Windows x64 con Rust estable, MSVC o GNU, compilador de recursos, Python 3.11+ y `tar`. Para regenerar el puente de AirPlay: target `wasm32-unknown-unknown` y `wasm-bindgen-cli` 0.2.130. Los archivos generados del cliente están incluidos en el árbol.

```powershell
./client.ps1 -Action build
./dev.ps1 -Action test
./dev.ps1 -Action lint
./desktop.ps1 -Action test
./desktop.ps1 -Action lint
./desktop.ps1 -Action test -Binary megalodon-setup
./package.ps1
./Instalar.ps1 -VerifyOnly
```

`package.ps1` descarga las dependencias fijadas y verifica sus hashes, compila ambos ejecutables, crea el paquete con una lista explícita de archivos y ejecuta la verificación interna del setup. El código fuente se distribuye junto al binario. La publicación todavía necesita una decisión de firma y una revisión de derechos sobre los avatares de terceros antes de presentarlos como recursos propios reutilizables.

Referencias de los flujos: [checkout](https://github.com/actions/checkout), [upload-artifact](https://github.com/actions/upload-artifact) y [entorno MSVC](https://github.com/ilammy/msvc-dev-cmd).
