# GitCraft — Etapa E2: índice y commits locales

**Fecha:** 2026-11-21  
**Rama:** develop (únicamente).  
**Rollback previo:** d9482955da7783a3b2894df52c98584b25ddbf84 (E1 validada).  
**main:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4 (intacta).  
**Estado:** IMPLEMENTADO, PENDIENTE DE COMPILACIÓN Y PRUEBAS E2.

## Alcance

Se mantiene el lector Git E1 y se añaden:
1. Diff por archivo, separado entre staged y unstaged.
2. Preparar un archivo individual (stage).
3. Retirar un archivo del índice (unstage), conservando su contenido.
4. Crear un commit local únicamente con el índice preparado.
5. Vista previa y confirmación separadas antes de cualquier operación que escriba.

No se añaden reset, clean, checkout, rebase, merge, push, pull ni fetch.

## Protección de datos

- La vista previa y la confirmación son dos acciones separadas.
- El motor vuelve a comparar el índice, HEAD y estado del trabajo antes de escribir. Para archivos, compara también la huella de sus contenidos.
- Cambios entre previsualización y confirmación se rechazan.
- Sólo admite rutas individuales que aparecen en el estado Git, nunca carpetas ni rutas externas. Renombrados, conflictos y symlinks quedan fuera del alcance E2.
- Se pasa la ruta después de -- sin ejecutar shell.
- Stage prepara el archivo completo, no líneas individuales. Para un archivo no seguido, la vista previa avisa de que no muestra su contenido.
- Unstage necesita un HEAD con al menos un commit.
- No se crean commits en HEAD separado, durante merge/cherry-pick/revert, sin cambios preparados ni sin mensaje.
- Un commit E2 no añade automáticamente archivos ni contacta con GitHub.
- Para evitar efectos inesperados de programas externos, el commit E2 usa hooks y firma GPG deshabilitados sólo durante esa orden. **No ejecuta las validaciones de pre-commit propias del repositorio ni firma el commit.** La interfaz lo advierte; revisar esta política si se requieren validaciones obligatorias.
- No se guardan en preferencias acciones pendientes ni mensajes de commit.
- La protección contra cambios simultáneos es una comparación de estado inmediatamente antes de escribir, no un bloqueo transaccional. No trabajar sobre el mismo repositorio desde otras aplicaciones durante una operación E2.
- Las pruebas manuales se realizan exclusivamente en un repositorio temporal, **nunca en Samafra/gitcraft ni en producción**.

## Archivos modificados o añadidos

- src/local_git_actions.rs — motor y tests de stage/unstage/commit.
- src/local_git.rs — estado y diffs por archivo.
- src/ui_local_git.rs — vista previa y botones de confirmación.
- src/app.rs — estados y respuestas asíncronas.
- src/lib.rs — módulo nativo E2.
- locales/es.json y locales/en.json — traducciones.
- scripts/validar_e2_local.sh — controles locales seguros.

## 1. Validación automática macOS

Con GitCraft cerrada:

~~~bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
~~~

Sólo continuar si aparece develop y el árbol está limpio.

~~~bash
git fetch origin
git pull --ff-only origin develop
git log -1 --oneline
cargo fmt --all -- --check
~~~

Si Rustfmt muestra diferencias, detenerse y comunicar la salida para corregirla en develop. No ejecutar formateo ni commit masivo sin revisión.

Si el formato es correcto:

~~~bash
set -o pipefail
bash scripts/validar_e2_local.sh 2>&1 | tee /tmp/gitcraft-e2-validacion.log
~~~

La validación compila el programa, ejecuta los tests E2 en repositorios temporales, verifica regresiones E0/E1, Clippy y WebAssembly si está instalado. No ejecuta GitHub Actions ni modifica ramas.

**Resultado esperado cuando se verifique:** OK: controles automáticos E2 completados; faltan pruebas manuales.

## 2. Crear un repositorio de pruebas independiente

En otra Terminal:

~~~bash
E2_TEST="$(mktemp -d /tmp/gitcraft-e2-manual.XXXXXX)"
git -C "$E2_TEST" init -q
git -C "$E2_TEST" symbolic-ref HEAD refs/heads/prueba-e2
git -C "$E2_TEST" config user.name "GitCraft E2 prueba"
git -C "$E2_TEST" config user.email "gitcraft-e2@example.invalid"
printf 'texto original\n' > "$E2_TEST/ejemplo.txt"
git -C "$E2_TEST" add -- ejemplo.txt
git -C "$E2_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Inicial E2"
printf 'texto actualizado\n' > "$E2_TEST/ejemplo.txt"
printf 'archivo nuevo\n' > "$E2_TEST/archivo nuevo.txt"
printf 'Repositorio temporal: %s\n' "$E2_TEST"
~~~

Conservar la ruta impresa. No sustituirla por la ruta del repositorio real GitCraft.

## 3. Pruebas visuales

Abrir la aplicación con cargo run --locked, entrar en Git local sin token y abrir la ruta temporal.

1. Confirmar que ejemplo.txt está modificado y archivo nuevo.txt aparece como no seguido.
2. Consultar el diff individual del archivo modificado.
3. Pulsar Preparar en ejemplo.txt. Debe aparecer la vista previa antes de escribir.
4. Confirmar la vista previa: el archivo pasa a staging.
5. Pulsar Retirar del índice y confirmar: debe conservarse el texto editado.
6. Preparar de nuevo ejemplo.txt y archivo nuevo.txt uno por uno.
7. Introducir el mensaje Prueba local E2 y pulsar Vista previa del commit.
8. Comprobar rama, mensaje y cantidad de archivos antes de confirmar.
9. Confirmar: el historial muestra el nuevo commit local y no hay push.
10. Para verificar una confirmación obsoleta, modifica de nuevo ejemplo.txt en el repositorio temporal desde Terminal, vuelve a abrirlo o actualízalo en GitCraft y pulsa Preparar para generar su vista previa. Sin confirmar todavía, añade otra línea al mismo archivo desde Terminal. Al pulsar Confirmar operación, GitCraft debe rechazarla sin preparar esa nueva versión.

Contrastar en Terminal:

~~~bash
git -C "$E2_TEST" status --short
git -C "$E2_TEST" log -1 --oneline
~~~

Después de cerrar GitCraft, verificar que el proyecto real sigue en develop y limpio:

~~~bash
cd ~/Projects/gitcraft
git status -sb
git log -1 --oneline
~~~

No borrar el repositorio temporal antes de revisar el resultado.

## Compatibilidad y criterio de cierre

| Plataforma | Estado |
| --- | --- |
| macOS Intel | Implementada, validación E2 pendiente |
| macOS Apple Silicon | Pendiente |
| Linux | Pendiente |
| Windows | Pendiente de revisión específica de rutas y hooks |
| WebAssembly | Sin motor nativo Git; pendiente de validar |

E2 no se declarará validada hasta completar los controles automáticos y las pruebas manuales sobre un repositorio temporal. No se integrará en main sin autorización expresa y punto de recuperación comprobado.
