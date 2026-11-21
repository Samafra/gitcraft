# Reglas obligatorias de desarrollo — GitCraft

Estas instrucciones se aplican a todo cambio de código, prueba o documentación en este repositorio.

## Protección del trabajo

1. `main` es el punto de partida estable. Desarrollar y corregir **exclusivamente en `develop`**. No modificar `main` sin autorización expresa.
2. Antes de cualquier integración o sincronización, comprobar el estado de las ramas y registrar un punto de recuperación verificable. Sin `force push`, reescritura de historia ni operaciones destructivas sin autorización.
3. No utilizar ni ejecutar **GitHub Actions** para probar, compilar o publicar. Priorizar pruebas locales. No declarar una plataforma validada sin pruebas ejecutadas en ella.
4. Preservar las funciones existentes de GitHub, MCP, idiomas e interfaz, así como las capacidades macOS Universal, Linux, Windows y WebAssembly que se hayan verificado.
5. El objetivo funcional es un cliente **Git** comparable a SourceTree. **No implementar Mercurial.**
6. Las acciones que puedan perder trabajo (reset hard, discard, clean, rebase, force push y similares) deben tener previsualización y confirmación explícita.

## Bloqueo obligatorio de formato y compilación **ANTES** de publicar

**Problema corregido:** no se admite volver a subir archivos Rust sin formato para
que el usuario encuentre diferencias de Rustfmt después de sincronizar. Ese
procedimiento desperdicia tiempo y no constituye una validación de entrega.

1. **Antes de cualquier commit o publicación con cambios en `.rs`**, la persona
   o agente que prepara el código debe trabajar en un entorno que disponga
   realmente de la toolchain Rust y ejecutar, en este orden:
   `cargo fmt --all`, `cargo fmt --all -- --check`, `cargo check --locked`,
   `cargo test --locked` y `cargo clippy --locked --all-targets`.
   **Todos deben terminar correctamente antes del commit**; no basta con
   inspeccionar estáticamente la sintaxis ni con aplicar formato manualmente.
2. Si la herramienta del agente sólo permite escribir en GitHub por API y
   **no puede ejecutar Rustfmt sobre el contenido definitivo**, queda
   **prohibido publicar cambios nuevos de código Rust en `develop`**.
   Preparar una propuesta o un parche para ejecución desde un entorno
   local que sí tenga Rustfmt. No trasladar el error al usuario pidiéndole
   que formatee un commit ya publicado.
3. Única excepción documental al punto anterior: aplicar **literalmente un
   diff de Rustfmt que el usuario ya ha ejecutado** para corregir formato
   existente, comprobando que no cambia lógica ni ramas. Esto no cuenta
   como validación local realizada por el agente.
4. `scripts/preparar_rust_para_entrega.sh` ejecuta el formateo y los
   controles automáticos de forma local, **sin commit ni push**. Ejecutarlo
   **antes de publicar**, nunca como reparación después.
5. Los ganchos opcionales `.githooks/pre-commit` y
   `.githooks/pre-push` bloquean commits y pushes **locales** sin
   `cargo fmt --check`. Se pueden activar de forma voluntaria con
   `bash scripts/activar_guardas_formato.sh` (no alteran ramas).
   **Los hooks locales no interceptan la API GitHub** y por sí solos no
   sustituyen las prohibiciones anteriores.
6. No utilizar GitHub Actions. Si no hay ejecutor Rust accesible, detener la
   publicación de código e informar claramente de esa limitación.

## Norma estricta de comentarios — máxima claridad para principiantes en Rust

**Todo código nuevo o modificado debe estar ampliamente comentado en español, de modo que lo pueda entender una persona sin conocimientos previos de Rust.** La explicación ha de ser didáctica y clara, sin dar por conocidas las particularidades del lenguaje.

- Añadir un encabezado explicativo a cada módulo o archivo de código importante: qué hace, cuándo se utiliza y con qué módulos se relaciona.
- Usar comentarios de documentación Rust (`///`) en **cada función, método, struct, enum, trait y elemento público**, explicando su cometido, sus entradas y salidas, y sus posibles errores o efectos secundarios. Documentar también las funciones privadas con lógica significativa.
- Comentar cada bloque de lógica relevante, cada decisión condicional importante, bucle complejo, validación, transformación de datos y paso de una operación Git, indicando **qué hace y por qué**.
- Explicar con palabras sencillas los conceptos propios de Rust cuando aparezcan: `Result`, `Option`, `Some`, `None`, `Ok`, `Err`, referencias, préstamos, propiedad, `clone`, `async/await`, canales, `Arc`, `Mutex`, `cfg`, etc. No repetir la misma explicación innecesariamente si ya está documentada cerca.
- Explicar cómo se gestionan los errores, los estados asíncronos, las credenciales, las operaciones que cambian archivos o ramas y las diferencias entre escritorio y WebAssembly.
- En los tests, indicar de forma legible qué escenario se prepara, qué se ejecuta y qué se comprueba, especialmente en casos de error y prevención de pérdida de datos.
- Si se modifica código anterior, revisar y completar los comentarios del área afectada para que continúen siendo correctos.
- Favorecer nombres descriptivos, ejemplos pequeños cuando aporten claridad y comentarios próximos a la lógica a la que se refieren. **No sustituir código legible por comentarios ni añadir afirmaciones redundantes, desactualizadas o engañosas.**
- Nunca incorporar tokens, contraseñas, datos personales ni valores de secretos en comentarios, ejemplos o logs.

**Criterio de aceptación:** una revisión de cambios debe comprobar tanto la corrección funcional como la comprensión del código por alguien que no sabe Rust. Los comentarios son parte obligatoria de la entrega, no una tarea opcional posterior.

La auditoría y el plan de etapas inicial están en `docs/AUDITORIA_TECNICA_INICIAL.md`.
