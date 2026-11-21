# GitCraft — E3-A: historial avanzado en modo solo lectura

**Fecha:** 2026-11-21  
**Rama:** develop únicamente.  
**Punto de recuperación previo:** 40b7eb55812756acc60f3af76be17839e6ed8252 (cierre E2 macOS Intel).  
**main estable:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4.  
**Estado:** IMPLEMENTADO en develop; pruebas automáticas y visuales PENDIENTES.

## Alcance del roadmap

La auditoría técnica define E3 como historial y herramientas avanzadas. Se divide
en subetapas para evitar activar simultáneamente operaciones destructivas.

**E3-A incluye exclusivamente lectura:**
- Grafo ASCII de los últimos 80 commits de todas las referencias locales,
  generado por Git y presentado en un panel monoespaciado.
- Selección de dos commits y comparación de su contenido sin cambiar de rama.
- Blame por archivo seguido por Git, con autor y fecha de HEAD.
- Reflog de HEAD (últimos 40 movimientos).
- Etiquetas y lista de stash, sin posibilidad de aplicar o borrar nada.
- Uso sin token GitHub.

**E3-A no incluye:** reset, checkout, merge, rebase, cherry-pick, revert,
crear o eliminar tags, aplicar stash, manipular worktrees, LFS ni submódulos.
Las funciones remotas nativas aplazadas de E2 (clone, fetch, pull, push)
tampoco se incorporan aquí. Estas operaciones requieren fases específicas.

## Protección y límites

- No existen comandos Git de escritura en el motor de E3-A.
- Todos los argumentos se pasan individualmente, sin shell ni credenciales.
- Los SHA de comparación deben ser hashes válidos de objetos commit.
- El selector blame muestra archivos seguidos por Git; se validan rutas y
  pertenencia exacta al repositorio.
- Se desactivan diffs externos, conversores y descargas implícitas Git.
- Los resultados largos se recortan a 120 000 caracteres para la interfaz.
  El proceso Git todavía puede consumir memoria antes de ese recorte.
- El estado es temporal y no se escribe en preferencias.
- Las respuestas antiguas se invalidan al abrir otro repositorio o después
  de una operación E2 que modifique el índice o HEAD.
- El motor sólo funciona en aplicaciones de escritorio; WebAssembly conserva
  su interfaz sin acceso a procesos Git locales.

## Archivos

- src/local_git_history.rs — motor E3-A y siete tests sobre fixtures temporales.
- src/ui_local_history.rs añade además dos tests puros de selección y revocación de estado.
- src/ui_local_history.rs — interfaz gráfica, selectores y estado de memoria.
- src/app.rs — eventos asíncronos con comprobación de generación.
- src/ui_local_git.rs — integración tras las herramientas E1/E2.
- src/lib.rs — módulos condicionados a escritorio.
- locales/es.json y locales/en.json — traducciones.
- scripts/validar_e3a_local.sh — validación reproducible sin GitHub Actions.

## Verificación estática previa y comandos Git

Antes de solicitar la ejecución en macOS, se realizó una revisión estática
de los archivos Rust y la consistencia de las traducciones ES/EN. Además se
ejecutaron directamente los comandos Git de E3-A sobre repositorios
temporales en el entorno de verificación Linux (Git 2.47.3): grafo, listado
de revisiones con separadores NUL, diff, blame con ruta Unicode, reflog,
etiquetas, stash y lectura sobre un repositorio sin commits. No se detectó
un fallo en esos comandos.

**Límite:** estas verificaciones no compilan el nuevo código Rust ni prueban
la interfaz gráfica. No acreditan todavía E3-A en macOS Intel ni en
otras plataformas. Rustfmt, Cargo y las pruebas visuales son obligatorios.

## Pruebas automáticas en macOS Intel

Con GitCraft cerrado:

~~~bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
~~~

Sólo si estás en develop y el árbol está limpio:

~~~bash
git fetch origin
git pull --ff-only origin develop
git log -1 --oneline
cargo fmt --all -- --check
~~~

Si Rustfmt da diferencias, detenerse y compartirlas. No formatear ni
confirmar automáticamente cambios sobre el repositorio de trabajo.

Si el formato ha pasado:

~~~bash
set -o pipefail
bash scripts/validar_e3a_local.sh 2>&1 | tee /tmp/gitcraft-e3a-validacion.log
~~~

Resultado esperado, todavía no confirmado:

~~~text
OK: controles automáticos E3-A completados; faltan pruebas visuales.
~~~

Los tests crean repositorios temporales con commits, etiquetas y stash; sólo
ellos escriben en sus carpetas privadas de prueba.

## Repositorio temporal para las pruebas visuales

Después de superar los controles de Cargo, preparar un repositorio
independiente para no usar operaciones de GitCraft sobre el código real:

~~~bash
E3_TEST="$(mktemp -d /tmp/gitcraft-e3-manual.XXXXXX)"
git -C "$E3_TEST" init -q
git -C "$E3_TEST" symbolic-ref HEAD refs/heads/prueba-e3
git -C "$E3_TEST" config user.name "GitCraft E3 prueba"
git -C "$E3_TEST" config user.email "gitcraft-e3@example.invalid"
printf 'primera línea\n' > "$E3_TEST/ejemplo.txt"
git -C "$E3_TEST" add -- ejemplo.txt
git -C "$E3_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Primer commit E3"
git -C "$E3_TEST" tag etiqueta-e3
printf 'segunda línea\n' > "$E3_TEST/ejemplo.txt"
git -C "$E3_TEST" add -- ejemplo.txt
git -C "$E3_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Segundo commit E3"
printf 'cambio reservado\n' >> "$E3_TEST/ejemplo.txt"
git -C "$E3_TEST" stash push -m "Reserva temporal E3"
printf 'RUTA TEMPORAL E3: %s\n' "$E3_TEST"
~~~

Este bloque crea **únicamente un repositorio de prueba aislado**. Conservar
la ruta mostrada; no sustituirla por la ruta de Samafra/gitcraft.

## Pruebas visuales después de validar Cargo

1. Abrir GitCraft con cargo run --locked, sin token GitHub.
2. En Git local, abrir un repositorio existente con dos o más commits.
3. Desplegar Historial avanzado, pulsar Actualizar y comprobar el grafo.
4. Elegir dos commits diferentes y contrastar el diff con git diff en Terminal.
5. Seleccionar ejemplo.txt, consultar blame y comparar con git blame HEAD.
6. Consultar reflog, etiqueta-e3 y la entrada Reserva temporal E3 en stash;
   no debe existir ningún control para aplicar o eliminar estas referencias.
7. Cambiar a otro repositorio; el panel debe descartar datos anteriores.
8. Comprobar con git status -sb y git log -1 que el repositorio no cambió.

## Matriz y criterio de cierre

| Plataforma | Estado E3-A |
| --- | --- |
| macOS Intel | Implementada, pruebas automáticas y visuales pendientes |
| macOS ARM | Pendiente |
| Linux | Pendiente |
| Windows | Pendiente |
| WebAssembly | Motor local excluido, compilación pendiente |

E3-A no se declarará validada hasta tener fmt, check, tests, Clippy y
pruebas visuales correctas en macOS Intel. E3 completo seguirá pendiente.
No fusionar en main sin autorización expresa y rollback comprobado.
