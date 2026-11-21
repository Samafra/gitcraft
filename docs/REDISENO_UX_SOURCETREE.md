# GitCraft — Rediseño UX tipo SourceTree (antes de E3-C)

**Fecha:** 2026-11-21  
**Rama:** develop exclusivamente  
**Punto de recuperación:** 4ccdf018d0f8ffefb679901d73866db973830a01 (E3-B validada en macOS Intel)  
**main estable:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4  
**Estado:** IMPLEMENTADO EN DEVELOP; FORMAT, CARGO Y PRUEBAS VISUALES PENDIENTES.

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

**Resultado esperado (no confirmado):**

~~~text
OK: controles automáticos UX superados; pruebas visuales pendientes.
~~~

El script comprueba compilación, tests de interfaz, regresiones E0–E3-B,
Clippy y, condicionalmente, WebAssembly; controla que no cambie HEAD.

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

## Criterios de cierre

La interfaz queda **implementada pero no validada** hasta superar Cargo,
Rustfmt, Clippy, regresiones y pruebas visuales en macOS Intel.
E3-C no comienza hasta cerrar la etapa de rediseño. No se integra
en main sin autorización expresa y rollback comprobado.
