# GitCraft — Auditoría técnica inicial y plan de evolución Git

**Fecha:** 2026-11-21  
**Repositorio:** `Samafra/gitcraft`  
**Rama auditada:** `develop`  
**Punto de partida y rollback:** `main` en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`  
**Estado:** revisión estática de código; sin compilación ni pruebas de ejecución.

## 1. Reglas del proyecto

1. `main` se mantiene como referencia estable. Todo cambio se realiza en `develop`.
2. Antes de integrar o sincronizar se comprueban las dos ramas y se fija un punto de rollback; integración solo con avances verificables, nunca `force push`.
3. No ejecutar GitHub Actions. Validaciones locales y revisión manual.
4. Preservar funciones existentes de GitHub, interfaz `egui/eframe`, MCP e idiomas. Mantener compatibilidad macOS Universal (Intel/Apple Silicon), Windows, Linux y WebAssembly **según capacidades verificables**.
5. Objetivo: cliente **Git** de funcionalidad comparable a SourceTree. **Mercurial queda expresamente excluido.**
6. No aplicar operaciones destructivas (reset hard, discard, clean, force push, rebase en curso) sin previsualización y confirmación explícita.
7. No dar por válida una plataforma sin pruebas ejecutadas en esa plataforma.

## 2. Inventario actual

- Rust 2021, `egui/eframe` 0.29, `reqwest` 0.12, `tokio` en escritorio, `wasm-bindgen-futures` en Web. Véase `Cargo.toml`.
- `src/app.rs`: estado UI, tareas asíncronas y pestañas.
- `src/github.rs`: autenticación por token y listado de repositorios GitHub.
- `src/github_manage.rs`: PR, Issues, Actions y Releases, mediante API GitHub.
- `src/repo_ops.rs`: ramas remotas, exploración de ficheros y Commit & Push remoto mediante Contents API.
- `src/ui_*.rs`: pantallas y cliente MCP.
- `src/mcp_client.rs` y `src/mcp_server.rs`: transportes/servidor MCP.
- `locales/es.json`, `locales/en.json`: traducciones.
- `.github/workflows/release.yml`: workflow existente; **no se ha ejecutado**.
- No consta en `Cargo.toml` un backend Git local (`git2`, `gix` u otro), ni hay módulo propio que invoque `git` para gestionar un repositorio local.
- En el árbol de ficheros no existen ficheros de test separados ni una estructura `tests/`. La revisión de código no sustituye a `cargo test`.

**Conclusión:** GitCraft es actualmente un gestor de GitHub remoto con MCP; para acercarse a SourceTree debe incorporar un motor Git local independiente de GitHub.

## 3. Hallazgos priorizados

### P0 — Credenciales persistentes en el estado de UI

`GitCraftApp.saved_token` es un `String` serializable (`src/app.rs` líneas 39-46), y `save()` persiste el estado de la aplicación (línea 212). No se observa un almacén seguro de secretos. **Riesgo:** exposición del PAT en el almacenamiento de preferencias, incluido el contexto web si se persiste allí. **Acción:** excluir secretos del estado serializado, separar credenciales de configuración e integrar almacenes seguros de sistema en escritorio; revisar estrategia OAuth/web antes de guardado persistente.

### P0 — Token inyectado indiscriminadamente a procesos MCP

`src/mcp_client.rs` líneas 120-124 inyecta `GITHUB_PERSONAL_ACCESS_TOKEN` y `GITHUB_TOKEN` **a cualquier servidor Stdio** que se lance. `src/ui_mcp.rs` ofrece además presets de servidores no relacionados con GitHub (filesystem, memory). **Riesgo:** un proceso MCP de terceros puede recibir el PAT sin necesitarlo. **Acción:** envío de credenciales por servidor, autorización explícita, permisos mínimos, y ninguna propagación implícita a servidores genéricos.

### P1 — Creación de rama deja seleccionada la predeterminada

`src/app.rs`: al gestionar `BranchCreated(Ok)` (línea 240) se asigna `current_branch` a la nueva rama, pero se invoca `load_branches_and_root`, que en línea 133 restablece `current_branch` a `default_branch`. **Acción:** separar recarga de rama seleccionada y selección inicial; añadir test de regresión.

### P1 — Respuestas API erróneas pueden aparecer como listas vacías

`src/github_manage.rs` en `fetch_management_data` convierte varias respuestas con `.json().await.unwrap_or_default()` sin comprobar el HTTP status. **Riesgo:** un 403/429, token sin permisos u otra respuesta incorrecta puede parecer «sin PR/Issues/Actions/Releases». **Acción:** comprobar status, preservar errores, incluir contexto y pruebas de fallos HTTP.

### P1 — Respuestas asíncronas carecen de identidad de petición

`src/app.rs` utiliza un canal de `AsyncEvent` global con resultados de branches/directorio/ficheros sin identificador de repositorio, rama o generación (líneas 26-37 y 214-245). **Riesgo:** al cambiar rápidamente de repositorio, las respuestas tardías pueden actualizar la pantalla equivocada. **Acción:** contexto inmutable por solicitud, identificadores/generaciones y descarte de respuestas obsoletas.

### P1 — Ausencia de operaciones Git locales

`src/repo_ops.rs` implementa API remota de GitHub, no un working tree. No hay implementación para `status`, índice/staging, commit local, diff de trabajo, log gráfico, clone/fetch/pull/push, merge, rebase, stash, reflog, cherry-pick, worktree, tags, submódulos o LFS locales. **Acción:** motor Git de escritorio en módulo separado, independiente del servicio remoto.

### P2 — Paginación y límites parciales

`src/github.rs` carga como máximo 100 repositorios en una petición; `src/repo_ops.rs` solicita hasta 100 ramas; `src/github_manage.rs` limita PR/Issues a 50, Actions/Releases a 20, sin navegación completa. **Acción:** paginación explícita, lectura incremental y pruebas con conjuntos mayores.

### P2 — Peticiones por entrada y límites de API

`fetch_directory` en `src/repo_ops.rs` consulta el último commit de cada archivo por separado (líneas 95-187). En directorios grandes aumenta las peticiones, latencia y riesgo de rate limiting. **Acción:** limitar concurrencia, cachear metadatos y mostrar degradación/errores de forma clara.

### P2 — MCP HTTP/SSE y ejecución de comandos requieren endurecimiento

`src/mcp_client.rs` envía solicitudes HTTP independientes sin gestionar sesiones de transporte; intenta analizar la respuesta como JSON directo pese a aceptar `text/event-stream`. En Stdio usa `split_whitespace()` para dividir comando y argumentos, lo que rompe rutas/argumentos entrecomillados con espacios. Además crea y mata un proceso por llamada. **Acción:** protocolo de sesión verificable, parsing correcto de SSE, proceso reutilizable cuando corresponda, ejecución sin shell y con argumentos estructurados.

### P2 — Disponibilidad local indebidamente ligada al token GitHub

`src/app.rs` líneas 288-295 bloquea todas las pestañas cuando `user` es `None`. Un cliente Git debe permitir abrir y operar repositorios locales sin autenticarse con GitHub. **Acción:** navegación y estado local independientes de la sesión GitHub.

### P2 — Matriz de compatibilidad todavía no demostrada

Hay código condicional `wasm32` y un workflow de empaquetado, pero **no se han ejecutado** compilaciones ni pruebas de las cuatro plataformas en esta auditoría. WebAssembly no dispone de acceso equivalente a procesos Git/FS nativos; macOS Universal debe verificarse en Intel/Apple Silicon, incluyendo la versión mínima macOS deseada. **Acción:** definir matriz de capacidades y ejecutar pruebas locales por target.

## 4. Arquitectura objetivo propuesta

```text
GitCraft UI (egui/eframe)
 ├─ Workspaces / repos locales / historial / cambios / ramas
 ├─ Gestión remota GitHub existente (PR/Issues/Actions/Releases)
 └─ Interfaz y servidor MCP
          |
          v
 Application services (comandos, estado, DTOs, validación, permisos)
          |
          +-- LocalGitBackend [solo desktop: macOS, Windows, Linux]
          |     Git CLI mediante std::process/tokio::process, argumentos explícitos
          |     Sin invocar shell ni registrar secretos
          |     Opcional: librería Git Rust si justifica funcionalidad concreta
          |
          +-- GitHubService [API REST actual, refactorizada]
          |
          +-- WebBackend [WASM: API y capacidades del navegador]
          |
          +-- CredentialService [almacén seguro por plataforma]
```

**Criterio arquitectónico:** evitar introducir dos implementaciones distintas de la semántica Git sin necesidad. Empezar con adaptador `git` CLI en escritorio por cobertura de operaciones avanzadas; abstraerlo tras una interfaz testeable. Verificar disponibilidad y versión de Git, compatibilidad con SSH/HTTPS y soporte LFS. La versión WASM conserva y amplía las operaciones remotas permitidas y define explícitamente qué operaciones locales no puede realizar sin un agente autorizado.

## 5. Roadmap y puertas de aceptación

**E0 — Endurecimiento previo**
- Separar PAT del estado persistido y restringir propagación a MCP.
- Corregir selección de rama, propagación de errores y peticiones asíncronas obsoletas.
- Añadir tests unitarios y de integración para estos flujos; cero cambios en `main`.
- **Gate:** tests locales y pruebas manuales de regresión GitHub/MCP.

**E1 — Motor Git local: lectura**
- Abrir repositorio local sin token, detectar `git`, leer `status`, `HEAD`, ramas locales/remotas, historial DAG, detalles de commit y diffs sin escribir.
- Pruebas sobre repositorios temporales con commits y ramas conocidas.
- **Gate:** lecturas reproducibles y sin modificar `.git` ni working tree.

**E2 — Operaciones Git básicas controladas**
- Clonar, stage/unstage, commit, fetch, pull, push, checkout y ramas.
- Distinguir operaciones locales de `commit_file_push` remoto.
- Confirmaciones, vista previa, detección de conflictos, manejo de autenticación y errores.
- **Gate:** casos felices y de fallo con remotos locales de prueba.

**E3 — Historial y herramientas avanzadas**
- Grafo visual, comparador y blame, stash, tags, cherry-pick, merge, rebase, reset/revert, reflog, worktrees, Git Flow, LFS, submódulos y resolución de conflictos.
- **Gate:** escenarios de recuperación y preservación de datos demostrados.

**E4 — UX y multiplataforma**
- Diseño de paneles, navegación por repos, búsqueda y filtros, estado/operación en curso, atajos, i18n.
- macOS Intel/Apple Silicon, Windows y Linux; WASM con matriz de capacidades diferente.
- **Gate:** compilación y pruebas funcionales por plataforma; empaquetado local sin ejecutar GitHub Actions.

## 6. Pruebas mínimas previstas

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`, `cargo check` para targets configurados, donde el entorno lo permita.
- Fixtures: repo recién inicializado, limpio/sucio, rama divergente, archivo nuevo/modificado/eliminado, rutas con espacios y caracteres Unicode, conflictos deliberados, remotos SSH/HTTPS en entorno de pruebas.
- Seguridad: PAT no serializado, no filtrado en logs/procesos ajenos, herramientas MCP solo con credenciales necesarias.
- Integridad: verificación `git status`, `git log` y referencias SHA antes/después; ningún test destructivo sobre repos reales.
- Regressión: selección de rama creada, error 403/429, resultado obsoleto al alternar repos, paginación y archivos grandes.
- No se usarán GitHub Actions; no se crearán tags `v*` para disparar el workflow existente.

## 7. Evidencia y limitaciones

- Archivos auditados: `Cargo.toml`, `README.md`, `src/app.rs`, `src/github.rs`, `src/github_manage.rs`, `src/repo_ops.rs`, `src/mcp_client.rs`, `src/mcp_server.rs`, `src/ui_repos.rs`, `src/ui_changes.rs`, `src/ui_manage.rs`, `src/ui_mcp.rs`, `src/i18n.rs`, `src/main.rs`, `src/lib.rs`, `index.html`, `.gitignore`, `build.rs`, idiomas y workflow.
- Confirmado al comenzar: `main` y `develop` idénticas en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`.
- No se ha instalado ni ejecutado Cargo/Rust, no se han lanzado pruebas, no se ha clonado ni modificado código funcional ni desplegado releases.
- Las severidades son una clasificación de revisión estática; requieren validación en ejecución antes de afirmar vulnerabilidades explotables o compatibilidad.

**Siguiente acción recomendada:** implementar E0 en `develop`, empezando por aislamiento de credenciales y tests de regresión, manteniendo rollback explícito y `main` sin cambios.
