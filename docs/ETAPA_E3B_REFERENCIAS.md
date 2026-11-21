# GitCraft — E3-B: creación controlada de ramas y etiquetas locales

**Fecha:** 2026-11-21  
**Rama de implementación:** develop, exclusivamente.  
**Punto de recuperación anterior a E3-B:** 448f42cac545c6e5e39bb8f3da5a1f06097b4d0c (E3-A cerrada en macOS Intel).  
**Referencia estable main:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4.  
**Estado:** IMPLEMENTADO EN DEVELOP. Pruebas Rust y pruebas visuales macOS Intel PENDIENTES.

## Objetivo y límites de la subetapa

E3-A permite consultar el grafo, comparar commits, blame, reflog, tags y stash.
E3-B introduce las **primeras escrituras avanzadas de Git que no alteran
el directorio de trabajo**:

1. Crear una **rama local nueva** que señale uno de los commits seleccionados
   del historial E3-A, **sin hacer checkout** de esa rama.
2. Crear una **etiqueta ligera nueva** que señale un commit seleccionado,
   **sin firmarla, publicarla ni cambiar HEAD**.
3. Mostrar vista previa de la creación y exigir una segunda confirmación.
4. Rechazar nombres inválidos, refs existentes, SHA incorrectos y
   confirmaciones obsoletas.
5. Actualizar los datos de Git local tras cada creación satisfactoria.

No se admiten eliminación, renombrado o movimiento forzado de ramas/tags;
no hay checkout, reset, stash push/pop/apply, merge, rebase, cherry-pick,
revert, clone, fetch, pull ni push. Esas capacidades están pendientes de
fases posteriores y requieren controles específicos.

## Seguridad

- **El primer clic no escribe**: la vista previa captura los parámetros,
  HEAD, nombre simbólico de la rama activa, índice, estado de archivos,
  ramas locales y etiquetas.
- Antes de crear la referencia se comprueba de nuevo esta fotografía.
  Cualquier cambio hace fallar la confirmación.
- Los destinos se eligen de los SHA completos disponibles en el historial
  E3-A; Git confirma que el objeto existe y es un commit.
- Git check-ref-format valida nombres. Los nombres que comienzan con
  guion, valores con caracteres de control y rutas Git mal formadas
  se rechazan antes de mostrar la confirmación.
- Git branch y git tag se invocan **sin opciones force**, nunca sobrescriben
  una referencia existente y no cambian la rama activa.
- Se usa Git CLI con argumentos independientes sin ejecutar shell.
- Los hooks y la firma de etiquetas quedan deshabilitados exclusivamente
  para el comando de creación de referencia. **No se ejecutarán hooks
  personalizados del repositorio**; esa limitación aparece en la UI.
- Mientras se confirma E3-B, se desactivan otras escrituras locales E2
  y el cambio de repositorio desde la UI.
- Cualquier edición de la ruta visible revoca la vista previa.
- Tras crear una ref, los datos E3-A se invalidan y Git local se refresca.
- Los formularios y autorizaciones viven sólo en memoria.
- La comprobación de estado previa **no es un bloqueo transaccional**:
  durante el mínimo intervalo entre comparar y escribir otro programa
  podría modificar otra referencia. Git rechaza el mismo nombre ya existente,
  pero no se garantiza exclusión general. No usar dos clientes para modificar
  simultáneamente el mismo repositorio.
- La acción modifica de verdad las referencias Git: las pruebas manuales
  son **exclusivamente en repositorios temporales**.

## Archivos

- src/local_git_refs.rs: motor seguro, tipos, vista previa y ocho tests.
- src/ui_local_history.rs: formulario, resumen y doble confirmación E3-B.
- src/app.rs: eventos asíncronos, comprobaciones de identidad y recarga.
- src/ui_local_git.rs: guardas frente a escrituras simultáneas y cambio de ruta.
- src/lib.rs: módulo nativo E3-B ausente en WebAssembly.
- locales/es.json y locales/en.json: traducciones.
- scripts/validar_e3b_local.sh: validación sin GitHub Actions.

## 1. Validación automática en macOS Intel

Con GitCraft cerrado:

~~~bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
~~~

Si la rama es develop y el estado está limpio:

~~~bash
git fetch origin
git pull --ff-only origin develop
git log -1 --oneline
cargo fmt --all -- --check
~~~

**No ejecutar cargo fmt sin --check.** Si Rustfmt muestra diferencias,
se corregirá únicamente el formato en develop tras revisar el diff.

Si Rustfmt termina correctamente:

~~~bash
set -o pipefail
bash scripts/validar_e3b_local.sh 2>&1 | tee /tmp/gitcraft-e3b-validacion.log
~~~

Resultado esperado, aún no confirmado:

~~~text
OK: controles automáticos E3-B completados; faltan pruebas visuales.
~~~

El script ejecuta check, pruebas E3-B, regresiones E0/E1/E2/E3-A, Clippy,
y WASM si está instalado su target. Los tests Rust crean referencias
**sólo en sus propios repositorios temporales**. No se utilizan Actions.

## 2. Crear un repositorio TEMPORAL para pruebas visuales

En Terminal:

~~~bash
E3B_TEST="$(mktemp -d /tmp/gitcraft-e3b-manual.XXXXXX)"
git -C "$E3B_TEST" init -q
git -C "$E3B_TEST" symbolic-ref HEAD refs/heads/prueba-e3b
git -C "$E3B_TEST" config user.name "GitCraft E3-B prueba"
git -C "$E3B_TEST" config user.email "gitcraft-e3b@example.invalid"
printf 'versión original\n' > "$E3B_TEST/ejemplo.txt"
git -C "$E3B_TEST" add -- ejemplo.txt
git -C "$E3B_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Primer commit E3-B"
printf 'versión actualizada\n' > "$E3B_TEST/ejemplo.txt"
git -C "$E3B_TEST" add -- ejemplo.txt
git -C "$E3B_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Segundo commit E3-B"
printf 'REPOSITORIO TEMPORAL E3-B: %s\n' "$E3B_TEST"
git -C "$E3B_TEST" log -2 --oneline
~~~

**No sustituir esta ruta por ~/Projects/gitcraft.** La carpeta se
puede inspeccionar después; no es necesario eliminarla.

## 3. Pruebas visuales manuales

Abrir GitCraft con cargo run --locked, sin token GitHub, abrir Git local con
la ruta temporal y desplegar E3-A / Historial avanzado.

1. Pulsar Actualizar historial avanzado y verificar que se ven los dos commits.
2. En E3-B elegir Rama local, el nombre **rama/e3b** y el commit más nuevo.
   Solicitar vista previa: **todavía no debe existir la rama**.
3. Confirmar y comprobar que se muestra el aviso de creación. Git local
   debe seguir en prueba-e3b, sin cambiar archivos ni hacer checkout.
4. Volver a abrir el historial avanzado. Elegir Etiqueta ligera, nombre
   **etiqueta-e3b** y el commit más antiguo. Previsualizar y confirmar.
5. Comprobar que etiqueta-e3b existe y apunta al SHA antiguo.
6. Intentar de nuevo rama/e3b o etiqueta-e3b: GitCraft debe mostrar un
   error controlado y **no sobrescribir** la referencia previa.
7. Previsualizar una nueva etiqueta **no-debe-crearse**. Antes de confirmar,
   modificar ejemplo.txt desde Terminal en la carpeta temporal. Confirmar:
   **debe rechazar** la vista previa obsoleta. Los cambios del archivo
   deben conservarse.
8. Introducir un nombre con guion inicial (por ejemplo -forzar) y
   confirmar que GitCraft lo rechaza sin escribir.
9. Verificar que la ruta original de GitCraft sigue limpia, HEAD y
   contenido sin cambios, y que no se ha enviado nada a remoto.

En Terminal, contrastar:

~~~bash
git -C "$E3B_TEST" branch --show-current
git -C "$E3B_TEST" branch --list
git -C "$E3B_TEST" tag --list
git -C "$E3B_TEST" rev-parse refs/heads/rama/e3b
git -C "$E3B_TEST" rev-parse refs/tags/etiqueta-e3b
git -C "$E3B_TEST" log -2 --oneline
git -C "$E3B_TEST" status --short
~~~

Para la prueba 7, después de previsualizar en GitCraft:

~~~bash
printf 'cambio externo posterior\n' >> "$E3B_TEST/ejemplo.txt"
~~~

No usar reset ni restore para deshacer la prueba. Se conservará el
archivo modificado y se verificará que la etiqueta no existe:

~~~bash
git -C "$E3B_TEST" tag --list no-debe-crearse
~~~

Después de cerrar la aplicación:

~~~bash
cd ~/Projects/gitcraft
git status -sb
git log -1 --oneline
~~~

## Compatibilidad y cierre

| Plataforma | Estado E3-B |
| --- | --- |
| macOS Intel | Implementada; validación automática y visual pendientes |
| macOS Apple Silicon | Pendiente |
| Linux | Pendiente |
| Windows | Pendiente; comprobar semántica de hooks y rutas |
| WebAssembly | Sin motor Git local; compilación pendiente |

**Cierre E3-B:** sólo tras Rustfmt, check, los ocho tests E3-B,
regresiones, Clippy y pruebas manuales satisfactorias sobre un
repositorio temporal de macOS Intel. E3 completo continúa abierto.

No se modifica main ni se integra sin autorización expresa y
punto de recuperación comprobado.
