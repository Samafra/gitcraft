# GitCraft — Rediseño UX tipo SourceTree (antes de E3-C)

**Fecha:** 2026-11-21  
**Rama:** develop exclusivamente  
**Punto de recuperación:** 4ccdf018d0f8ffefb679901d73866db973830a01 (E3-B validada en macOS Intel)  
**main estable:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4  
**Estado:** VALIDACIÓN VISUAL PARCIAL — 8 de 10 comprobaciones correctas comunicadas por el usuario. **NO CERRADO**. Pendientes: contraste del tema claro y simplificación del Historial como lista de commits con detalles separados. Rustfmt, compilaciones, tests y Clippy previamente verificados según resultados comunicados.

## Motivo

El usuario considera la interfaz anterior difícil de leer, desorganizada y
complicada de navegar: múltiples funciones mezcladas en un panel muy largo,
pestañas dispersas y contraste insuficiente.

La referencia visual es SourceTree para macOS: barra de herramientas,
menú lateral por categorías y vistas de trabajo separadas para archivos,
historial y referencias. Se recrea la organización, no los logotipos ni
los recursos gráficos de Atlassian.

## Diseño

- **Cabecera:** GitCraft, repositorio abierto, rama actual, idioma, tema
  claro u oscuro de alto contraste.
- **Barra de acciones locales:** Commit, Historial, Nueva rama, Nueva
  etiqueta, Actualizar. Fetch, Pull, Push, Merge y Stash deshabilitados
  porque esos comandos todavía no están implementados con seguridad.
- **Lateral izquierdo ajustable:** Espacio de trabajo (Estado de archivos,
  Historial); Referencias (Ramas, Etiquetas, Remotos, Stash) y una lista de
  ramas locales sin checkout. GitHub y MCP aparecen en grupos separados.
- **Estado de archivos:** dos columnas: a la izquierda Sin preparar y
  Preparados; a la derecha diff seleccionado. El mensaje de commit va debajo
  y la vista previa E2 aparece encima de la lista para no pasar inadvertida.
- **Historial:** grafo, comparación y blame existentes de E3-A, expandidos
  por defecto.
- **Ramas y etiquetas:** listas de consulta y creación E3-B con doble
  confirmación en un lugar específico. No se hace checkout ni push.
- **Remotos y stash:** lectura de referencias locales; no hay acciones de
  escritura, recuperación ni descarte.
- **GitHub y MCP:** funciones anteriores accesibles en el lateral.
  MCP local no exige token GitHub; las páginas GitHub mantienen su permiso.
- **Paleta:** tema claro predeterminado y tema oscuro alternativo; ambos
  usan mayor contraste de texto, paneles y botones, con espaciado uniforme.

## Protección del código y compatibilidad

No se han añadido operaciones Git nuevas. Se reutilizan los mismos
motores E1 (lectura), E2 (stage, unstage y commit), E3-A (historial)
y E3-B (ramas y etiquetas). El cambio afecta a composición visual,
estado de navegación y traducciones. No hay merge en main, force push,
checkout, reset, clean, stash pop ni llamadas GitHub Actions.

En WebAssembly la estructura de navegación también se compila, pero los
comandos locales siguen excluidos. La validación de plataformas adicionales
queda pendiente.

**Archivos:**
- src/ui_shell.rs: nuevo shell, panel lateral y barra de acciones.
- src/ui_local_git.rs: categorías de archivos, diff y commit separados.
- src/ui_local_history.rs: historial por defecto expandido y formulario
  E3-B desacoplado del historial.
- src/app.rs: estado de la navegación y colores de mayor contraste.
- src/lib.rs: exportación del módulo de interfaz.
- locales/es.json y locales/en.json: navegación traducida.
- scripts/validar_ux_sourcetree.sh: verificaciones sin Actions.

## Verificación automática en macOS Intel

Con GitCraft cerrada:

~~~bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
~~~

Sólo si la rama es develop y no existen cambios pendientes:

~~~bash
git pull --ff-only origin develop
git log -1 --oneline
cargo fmt --all -- --check
~~~

Si Rustfmt muestra diferencias, DETENERSE y compartir la salida. No
ejecutar cargo fmt sin --check ni modificar archivos localmente.

Si el formato pasa:

~~~bash
set -o pipefail
bash scripts/validar_ux_sourcetree.sh 2>&1 | tee /tmp/gitcraft-ux-validacion.log
~~~

**Resultado automático comunicado:** el script pasó todas las comprobaciones salvo una diferencia de indentación detectada por Rustfmt. El usuario confirmó posteriormente que `cargo fmt --all -- --check` no muestra diferencias tras el commit `6bb7ce99`. No se ha repetido el script completo sobre dicho commit, que únicamente modifica espacios.

~~~text
OK: controles automáticos UX superados; pruebas visuales pendientes.
~~~

El script comprueba compilación, tests de interfaz, regresiones E0–E3-B,
Clippy y, condicionalmente, WebAssembly; controla que no cambie HEAD.

## Preparar un repositorio temporal para la interfaz

Este bloque crea datos de prueba sólo en una carpeta nueva de /tmp, nunca
en la carpeta GitCraft del proyecto. Mantener la variable UX_TEST en la
misma Terminal durante las verificaciones:

~~~bash
UX_TEST="$(mktemp -d /tmp/gitcraft-ux-manual.XXXXXX)"
git -C "$UX_TEST" init -q
git -C "$UX_TEST" symbolic-ref HEAD refs/heads/prueba-ux
git -C "$UX_TEST" config user.name "GitCraft UX Prueba"
git -C "$UX_TEST" config user.email "gitcraft-ux@example.invalid"
printf 'primera versión\n' > "$UX_TEST/ejemplo.txt"
git -C "$UX_TEST" add -- ejemplo.txt
git -C "$UX_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Primer commit UX"
printf 'segunda versión\n' > "$UX_TEST/ejemplo.txt"
git -C "$UX_TEST" add -- ejemplo.txt
git -C "$UX_TEST" -c core.hooksPath=/dev/null -c commit.gpgsign=false commit -qm "Segundo commit UX"
printf 'cambio sin preparar\n' >> "$UX_TEST/ejemplo.txt"
printf 'archivo nuevo\n' > "$UX_TEST/nuevo archivo.txt"
printf 'REPOSITORIO UX: %s\n' "$UX_TEST"
git -C "$UX_TEST" status --short
~~~

Estado esperado:

~~~text
 M ejemplo.txt
?? nuevo archivo.txt
~~~

Después de las pruebas, comprobar que GitCraft original sigue intacto:

~~~bash
cd ~/Projects/gitcraft
git status -sb
git log -1 --oneline
~~~

## Pruebas visuales después de superar Cargo

1. Abrir GitCraft con cargo run --locked. En 1200×780 comprobar la nueva
   cabecera y el lateral persistente, sin la antigua fila de ocho pestañas.
2. Alternar tema claro y oscuro; comprobar que texto, botones, listas,
   selección y controles deshabilitados se leen bien. Redimensionar a
   1000×680 y comprobar que se puede ajustar el ancho del lateral.
3. Abrir una carpeta Git temporal sin iniciar sesión en GitHub.
4. Verificar que los archivos modificados y no seguidos aparecen en
   Sin preparar y los preparados aparecen aparte.
5. Seleccionar un archivo: su diff debe mostrarse a la derecha.
   Preparar o retirar un archivo debe mostrar primero una vista previa
   arriba, sin escritura antes de la confirmación.
6. Comprobar que el commit sólo se puede previsualizar con al menos
   un archivo preparado y un mensaje escrito. Confirmar la operación
   exclusivamente en un repositorio temporal.
7. Abrir Historial y comprobar grafo, comparador y blame. Abrir
   Ramas y Etiquetas y localizar en su vista el formulario E3-B.
8. Abrir Remotos y Stash; verificar que son de lectura y no ejecutan
   checkout, fetch, push, borrado o stash pop.
9. Verificar que las páginas GitHub siguen disponibles en el lateral y
   requieren sesión. MCP local debe poder abrirse sin token GitHub.
10. Abrir otro repositorio y comprobar que no se muestran datos viejos
    ni previews obsoletas. Comprobar que el repositorio GitCraft
    original sigue intacto.

## Resultado de la primera ronda visual — 21/11/2026

El usuario confirma ocho comprobaciones satisfactorias sobre la interfaz:
barra y menú lateral, panel lateral redimensionable, separación
preparados/sin preparar, diff a la derecha, confirmaciones E2,
ubicación de Ramas/Etiquetas E3-B, accesos a remotos/stash/GitHub/MCP,
y aislamiento de datos al cambiar de repositorio.

**Pendientes expresos (2 de 10):**

- **Tema claro:** el texto y la gama de colores siguen siendo difíciles de
  leer. Hay que comprobar contraste de texto normal, secundario,
  encabezados, botones, filas seleccionadas y diferencias.
- **Historial:** la vista con grafo ASCII y varios bloques desplegables
  sigue siendo compleja. Se prefiere una **lista de commits tipo SourceTree**
  con las columnas esenciales y selección de una fila; el detalle y las
  diferencias deben aparecer separados debajo. Mantener comparación, blame
  y consultas anteriores disponibles sin ocupar la vista principal.

**Criterio de aceptación adicional:** no cerrar el rediseño sólo por tener
compilación correcta. Ambas áreas deben resultar fáciles de leer y usar
en el Mac Intel del usuario. No iniciar E3-C hasta recibir conformidad.

**Regla de publicación:** AGENTS.md prohíbe subir código Rust nuevo a
develop cuando el entorno no puede ejecutar Rustfmt y Cargo antes del
commit. Cualquier parche correctivo se prepara y verifica antes de
publicarlo; nunca se delega al usuario corregir formato tras el push.

## Criterios de cierre

La comprobación automática está **sustancialmente completada** en macOS Intel: compilación, seis tests de interfaz, 50 regresiones y Clippy superados en `7b0326b8`, y Rustfmt confirmado sin diferencias en `6bb7ce99`. El último cambio es exclusivamente de indentación. **El cierre está pendiente de las pruebas visuales**.
E3-C no comienza hasta cerrar la etapa de rediseño. No se integra
en main sin autorización expresa y rollback comprobado.
