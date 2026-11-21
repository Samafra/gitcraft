# GitCraft — E1: motor Git local de solo lectura

**Fecha:** 2026-11-21  
**Rama de trabajo:** develop  
**Punto de recuperación previo a E1:** a74a4ba00ad6906a9ab210ff287e3067c44c2c17 (cierre E0 macOS Intel).  
**main estable:** c208de70e87c7eabf5e5e1fea5546aa1bae43ef4.  
**Estado:** IMPLEMENTADO en develop; COMPILACIÓN Y PRUEBAS DEL NUEVO CÓDIGO PENDIENTES.

## Objetivo y alcance

Esta etapa introduce lectura de repositorios Git locales usando el Git instalado en el equipo, sin requerir autenticación ni contacto con GitHub. No se implementa Mercurial.

Las capacidades nuevas son:

1. Detectar y mostrar la versión del Git instalado.
2. Abrir la raíz de un repositorio o una subcarpeta, incluyendo rutas con espacios y el prefijo ~/.
3. Ver la rama actual, incluidos casos de HEAD separado.
4. Ver archivos modificados, preparados (staged), no preparados (unstaged), no seguidos y renombrados, usando git status --porcelain=v1 -z.
5. Ver las ramas locales y las referencias remotas ya guardadas localmente, sin ejecutar fetch.
6. Consultar los últimos 50 commits (SHA, autor, fecha y asunto).
7. Consultar el diff de cambios preparados, no preparados o de un commit del historial.

La pestaña «Git local» siempre está disponible incluso cuando no se ha conectado GitHub. Las pestañas remotas GitHub y el cliente MCP mantienen sus funciones previas. En WebAssembly sólo se muestra una explicación: el navegador no puede ejecutar el Git instalado en el ordenador.

## Protección del repositorio

- El motor no contiene commit, checkout, reset, clean, merge, rebase, fetch, pull ni push.
- Las invocaciones utilizan std::process::Command con argumentos separados, sin shell.
- Se desactivan los bloqueos opcionales de Git, fsmonitor y el paginador para evitar operaciones colaterales.
- En las diferencias se desactivan la ejecución de diff externos y textconv.
- Sólo se aceptan SHA hexadecimales de 40 o 64 caracteres para consultar un commit.
- Se usa el protocolo porcelain -z para interpretar nombres de archivo con espacios, caracteres Unicode y renombrados.
- El estado de Git local permanece en memoria; no se guarda la ruta ni la información de archivos en las preferencias.
- Cada consulta asíncrona lleva un identificador y los resultados antiguos se descartan si se ha cambiado de repositorio o de diff.
- Los diffs visibles se limitan a 160 000 caracteres; la implementación actual recoge la salida antes de recortarla.

## Archivos

- src/local_git.rs — motor de lectura y tests sobre repositorios Git temporales.
- src/ui_local_git.rs — interfaz en egui de E1.
- src/app.rs — integración de la pestaña, estados y respuestas asíncronas.
- src/lib.rs — módulos exclusivos de escritorio.
- locales/es.json y locales/en.json — nuevos literales.
- scripts/validar_e1_local.sh — validación reproducible, sin Actions.

## Validación local detallada en macOS

### 1. Sincronización segura

Con GitCraft cerrada, en Terminal:

~~~bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
git fetch origin
git rev-parse HEAD
git rev-parse origin/develop
~~~

Esperado: develop, sin cambios locales. Si existen archivos modificados, detenerse y revisarlos. Para sincronizar, sólo si el árbol está limpio:

~~~bash
git pull --ff-only origin develop
git log -1 --oneline
~~~

Nunca usar reset --hard, clean, force push ni modificar main.

### 2. Comprobar compilación y formato

~~~bash
cargo fmt --all -- --check
cargo check --locked
cargo test --locked local_git::tests -- --nocapture
cargo test --locked
cargo clippy --locked --all-targets
~~~

Si Rustfmt devuelve diferencias, **no** dar E1 por validada ni confirmar automáticamente cambios; guardar el diff, revisarlo y corregirlo en develop. Los tests temporales crean repositorios Git exclusivamente bajo el directorio temporal del sistema.

Si estos cinco comandos funcionan, ejecutar el script de auditoría:

~~~bash
bash scripts/validar_e1_local.sh 2>&1 | tee /tmp/gitcraft-e1-validacion.log
~~~

Esperado: OK: validación automática E1 local completada; faltan pruebas visuales.

### 3. Probar sin login GitHub

~~~bash
cargo run --locked
~~~

En la pantalla inicial, **no introducir ningún token**. Seleccionar «Git local», escribir la ruta de un repositorio, por ejemplo ~/Projects/gitcraft, y pulsar «Abrir».

Esperado: GitCraft muestra la raíz del repositorio, versión de Git, rama actual, estado, ramas e historial. El botón «Actualizar» vuelve a leer los datos sin hacer fetch.

### 4. Comparar con Git en Terminal

~~~bash
git -C ~/Projects/gitcraft status --short
git -C ~/Projects/gitcraft branch -a
git -C ~/Projects/gitcraft log -5 --oneline
~~~

La interfaz debe coincidir con estos resultados (el historial de la interfaz muestra hasta 50 commits). Si el repositorio está limpio, el estado puede aparecer vacío.

En GitCraft, pulsar «Cambios sin preparar» y «Cambios preparados». Abrir «Historial de commits», pulsar un hash y comprobar que aparece su diff. No se cambia HEAD ni la rama.

### 5. Casos adicionales

- Abrir una subcarpeta dentro del repositorio: GitCraft debe detectar la raíz.
- Introducir una carpeta que no sea Git: debe aparecer un error sin crear un repositorio.
- Abrir una ruta inexistente: error claro, sin crear carpetas.
- Abrir un repositorio sin commits: estado y ramas pueden consultarse con historial vacío.
- Cambiar rápidamente entre dos rutas de repositorio: una respuesta antigua no debe sobrescribir la nueva selección.
- Mantener GitHub desconectado durante todas estas comprobaciones: el acceso local debe seguir funcionando.

Para probar archivos modificados/staged/untracked sin tocar la copia real de GitCraft, utilizar los tests automáticos, que crean sus repositorios temporales propios.

### 6. Comprobar que no ha habido escrituras

Cerrar GitCraft y ejecutar:

~~~bash
git status --short
git branch --show-current
git log -1 --oneline
~~~

Esperado: sin modificaciones, develop y el mismo HEAD anterior a las pruebas.

## Compatibilidad por plataforma

| Plataforma | Diseño E1 | Verificación |
| --- | --- | --- |
| macOS Intel | Git CLI nativo | Pendiente de pruebas del usuario |
| macOS Apple Silicon | Git CLI nativo | Pendiente |
| Linux | Git CLI nativo | Pendiente |
| Windows | Git CLI nativo | Pendiente |
| WebAssembly | Vista informativa sin ejecución de Git local | Pendiente de compilación y revisión |

La arquitectura no implica que todas las plataformas estén verificadas; cada una necesita pruebas reales.

## Límites conocidos y etapas posteriores

- No hay escritura Git local ni acciones peligrosas en E1.
- No existe selector nativo de carpetas: se introduce la ruta en la interfaz.
- Los diffs de archivos no seguidos no aparecen en git diff; sí figuran en estado.
- Se muestran 50 commits; no hay paginación de historial.
- La comparación de commits se limita a un commit individual, no incluye selector de dos revisiones.
- No hay fetch/pull/push; las ramas remotas que se muestran son referencias ya conocidas por el repositorio.
- No se cargan repositorios bare en esta fase.
- El parser actual convierte nombres no UTF-8 con pérdida, para visualizarlos.
- En repositorios enormes, Git puede tardar en devolver estado y un diff grande puede consumir memoria antes de truncarse.
- La autenticación GitHub sigue gestionándose por separado; persistencia segura de tokens queda pendiente de una etapa específica.

## Criterio de cierre

E1 sólo podrá marcarse como validada en macOS Intel después de ejecutar sin fallos Cargo check/test/fmt/Clippy, las pruebas temporales, y las pruebas manuales de interfaz sin login. No se modifica main ni se integra código sin autorización explícita y punto de recuperación comprobado.
