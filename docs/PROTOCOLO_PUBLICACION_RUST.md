# GitCraft — Protocolo de publicación Rust sin diferencias de formato

**Vigente desde:** 2026-11-21  
**Rama de desarrollo:** develop, nunca main sin autorización.

## Incidencia y solución

Entre E2 y el rediseño UX aparecieron reiteradamente diffs de
\`cargo fmt --all -- --check\` después de haber publicado código Rust
a través de GitHub. Eso no es un defecto del código en ejecución:
es una **deficiencia del proceso de publicación**. El entorno de
administración remota usado para esos commits no dispone de Rustfmt.

**Regla principal:** no volver a publicar archivos \`.rs\` nuevos en
\`develop\` sin ejecutar Rustfmt sobre la versión exacta que se va a
confirmar. Si no hay un ejecutor local accesible, preparar el cambio
como propuesta o parche, sin publicar el código. Los diffs de formato
no se deben trasladar al usuario como trabajo posterior.

## Flujo exigido para el autor del cambio

Antes de realizar un commit Rust, en una copia local de develop:

~~~bash
bash scripts/preparar_rust_para_entrega.sh
~~~

Este comando **sí formatea** la copia local del autor, después comprueba
Rustfmt, compila, ejecuta tests y pasa Clippy. No añade archivos al índice,
no crea commits ni hace push. Es el autor quien debe revisar las
modificaciones formateadas y publicarlas sólo si todos los controles pasan.

Como salvaguarda adicional, los hooks opcionales de Git comprueban formato
justo antes de commits y pushes locales, en modo de solo lectura.

## Activar hooks de Git una sola vez

Desde develop:

~~~bash
bash scripts/activar_guardas_formato.sh
git config --local --get core.hooksPath
~~~

El segundo comando debe mostrar \`.githooks\`. La activación modifica
únicamente la configuración local de **este repositorio** y se niega a
sobrescribir un directorio de hooks previamente personalizado.

Los hooks nunca ejecutan \`cargo fmt\` automáticamente: **bloquean**
operaciones sin formato, sin cambiar archivos de forma sorpresiva.
Pueden desactivarse posteriormente sólo mediante una decisión explícita
de quien mantiene el repositorio; no se instalan de forma global.

## Limitación técnica importante

Los hooks no se ejecutan al crear commits mediante la API de GitHub.
Por ello, además de los hooks existe la prohibición expresa del
archivo \`AGENTS.md\`: ningún agente debe utilizar únicamente la API
de GitHub para publicar cambios nuevos en \`.rs\` sin haber ejecutado
Rustfmt y la batería de pruebas **antes** de publicar.

**Sin acceso a la ejecución de Rustfmt en el entorno del agente, no
se puede garantizar esta comprobación; el cambio Rust debe detenerse**
en vez de entregarlo con un formato sin validar.

Este mecanismo no utiliza GitHub Actions, no toca main y conserva el
punto de recuperación anterior al rediseño UX.
