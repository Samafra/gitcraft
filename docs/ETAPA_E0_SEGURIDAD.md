# GitCraft — E0: implementación de seguridad y estabilización

**Fecha:** 2026-11-21  
**Rama:** `develop` (exclusivamente)  
**Rollback previo:** commit `687579bc1715729b4ea7b649fda890ae79aa5cde` (informe y normas, antes de los cambios funcionales E0).  
**Referencia estable:** `main` en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`.

## Estado de la etapa

**Implementación aplicada: controles automáticos y siete comprobaciones funcionales básicas de macOS correctos**, según resultados comunicados por el usuario. El commit de formateo `a601a840a759b674ba3a9cdd6c92ec4dfc0af75d` está verificado en GitHub. **E0 todavía no está cerrada en su totalidad:** quedan pruebas de situaciones excepcionales y verificación de otras plataformas/targets. La revisión remota no ejecuta Cargo ni la interfaz gráfica.

## Cambios realizados

1. **Token GitHub en memoria:** `src/app.rs` marca `saved_token` con `#[serde(skip)]`. Se descarta el token de preferencias antiguas al deserializar; no se recupera automáticamente al reiniciar. Al desconectar se borra en memoria. No se ha implementado todavía un gestor Keychain/Credential Manager/libsecret; el usuario deberá volver a iniciar sesión tras reiniciar. **Importante:** si existe un fichero de preferencias anterior con un PAT, los bytes antiguos podrían persistir hasta que las preferencias se reescriban. Si hay sospecha de exposición, rotar el PAT en GitHub.
2. **MCP con consentimiento explícito:** `src/ui_mcp.rs` añade autorización temporal por endpoint Stdio. Editar destino o transporte revoca el permiso. `src/mcp_client.rs` elimina variables de entorno de GitHub heredadas por defecto, y sólo las agrega a procesos Stdio si hay consentimiento. Los servidores no relacionados no reciben credenciales automáticamente.
3. **Selección de rama tras creación:** `src/app.rs` diferencia entre selección inicial de repositorio y actualización de ramas. La rama nueva continúa seleccionada tras crearla. También se han corregido los botones de actualización para que no vuelvan a la rama predeterminada.
4. **Errores HTTP visibles:** `src/github_manage.rs` valida los códigos de respuesta y la deserialización JSON antes de actualizar PRs, Issues, Actions y Releases. Un 403/429 deja de convertirse en lista vacía.
5. **Respuestas obsoletas:** `src/app.rs` añade marcadores por petición para datos de ramas, carpetas, archivos, commits y gestión. Si se cambia de repositorio, rama o archivo, la respuesta anterior no sustituye los datos actuales. Una desconexión invalida también las respuestas de autenticación pendientes.
6. **Legibilidad:** los bloques nuevos y módulos afectados tienen comentarios ampliados en español, incluidos conceptos y escenarios de error, conforme a `AGENTS.md`.

## Pruebas automáticas añadidas

- El PAT no aparece en el JSON de preferencias y no se recupera de un estado antiguo.
- El PAT para MCP requiere autorización del usuario y del endpoint concreto.
- Una respuesta antigua o de otro repositorio no se considera vigente.
- La rama recién creada sigue seleccionada.
- Desconectar invalida la sesión pendiente y revoca el permiso MCP.
- Las respuestas HTTP simuladas 403 y 429 se notifican como errores.

**Estas pruebas fueron ejecutadas en el Mac del usuario como parte de `cargo test --locked` y el script de validación E0, según la salida comunicada.** No se dispone de un registro completo de las pruebas individuales en el entorno de revisión remota. Los servidores de prueba HTTP sólo escuchan en `127.0.0.1`.

## Cómo validar en macOS/Linux

```bash
git clone https://github.com/Samafra/gitcraft.git
cd gitcraft
git switch develop
bash scripts/validar_e0_local.sh
```

Si ya existe una copia local limpia, no es necesario clonar de nuevo: actualizar `develop` por fast-forward y ejecutar el script.

El script evita ejecutar en `main`, no fuerza cambios, no ejecuta Actions ni publica nada. Cargo puede crear artefactos ignorados bajo `target/`. La comprobación de WebAssembly sólo se intenta si `wasm32-unknown-unknown` está instalado. Para Windows hay que ejecutar los comandos equivalentes con Cargo y hacer pruebas manuales específicas.

## Evidencia de validación automática en macOS — 21/11/2026

El usuario ejecutó `scripts/validar_e0_local.sh` en su Mac y comunicó:

```text
OK: controles locales terminados sin errores; revisar aparte las pruebas manuales.
## develop...origin/develop
a601a84 (HEAD -> develop, origin/develop) style: aplicar rustfmt a fuentes Rust (E0)
```

Además, comunicó que `cargo fmt --all -- --check`, `cargo check --locked`, `cargo test --locked` y `cargo clippy --locked --all-targets` habían finalizado satisfactoriamente. Antes del commit, el usuario comparó los 11 archivos Rust formateados con un árbol temporal generado desde `HEAD` y confirmó que todos eran exclusivamente cambios de Rustfmt.

**Verificación remota independiente:** GitHub confirma que `develop` apunta a `a601a840a759b674ba3a9cdd6c92ec4dfc0af75d`, y que `main` continúa en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`. Este registro de validación se añade en un commit documental posterior.

**Alcance de esta evidencia:** controles automáticos correctos en el Mac del usuario. No acredita por sí sola pruebas manuales de interfaz ni compatibilidad macOS ARM, Linux, Windows o navegador WASM. El script sólo comprueba WebAssembly si el target está instalado; no consta el resultado específico de ese paso.

## Evidencia funcional macOS — 21/11/2026

El usuario ejecutó GitCraft en su MacBook Pro y comunicó **OK en las siete pruebas**, sin observaciones ni errores:

1. Arranque de la aplicación gráfica.
2. Autenticación GitHub y visualización de repositorios.
3. Actualización de ramas manteniendo seleccionada `develop`.
4. Apertura y lectura de archivos.
5. Desconexión del usuario.
6. Solicitud de token nuevamente al reiniciar la aplicación.
7. Permiso MCP inicialmente desactivado y revocado al cambiar de servidor.

Estas comprobaciones fueron realizadas manualmente por el usuario. No equivalen a una auditoría completa de seguridad ni a pruebas de todas las plataformas.

## Pruebas funcionales adicionales pendientes

- **Creación remota de una rama y permanencia de la selección** en un repositorio de prueba, sin tocar `Samafra/gitcraft/main`.
- **Respuesta obsoleta:** alternar rápidamente entre repositorios/ramas/archivos mientras las consultas siguen en curso y verificar que no se mezclan datos; desconectar durante una petición.
- **Errores de GitHub 403/429:** contrastar la presentación de errores con un escenario de prueba controlado; los tests automatizados de respuestas HTTP ya se ejecutaron satisfactoriamente en macOS según el usuario.
- **Credenciales MCP en procesos:** comprobar, con servidor de pruebas local y token ficticio, que los procesos no autorizados no heredan el token. La comprobación visual del consentimiento sí consta como OK.
- **Otras plataformas:** builds y pruebas manuales de macOS ARM, Windows, Linux y WebAssembly según corresponda; la arquitectura web no tiene las mismas capacidades nativas.

No ejecutar operaciones destructivas ni crear ramas o commits de prueba dentro de la rama estable.

## Verificación adicional realizada sobre el código fuente

- Se han releído los archivos modificados directamente desde `develop` y comprobado la correspondencia entre los tipos de `AsyncEvent`, sus emisores y sus receptores.
- Un comprobador estático de delimitadores (no un compilador Rust) no detectó llaves, paréntesis ni corchetes desparejados en los siete módulos Rust principales afectados.
- Se verificó en el código el uso de `#[serde(skip)]` para el PAT, la eliminación predeterminada de las variables de entorno del token en MCP y los nuevos tests de errores HTTP.
- Los tests del servidor HTTP simulado incluyen un límite de espera para no bloquear indefinidamente la validación.
- **La revisión remota no ha ejecutado Rust ni probado la aplicación gráficamente.** Sí se recibió el resultado satisfactorio de los controles locales del usuario en macOS. Las siete comprobaciones funcionales básicas fueron confirmadas posteriormente por el usuario; siguen pendientes los casos excepcionales y otros targets.

## Riesgos y asuntos pendientes fuera del alcance cerrado de E0

- Revisar e implementar almacenamiento seguro de credenciales nativo si se desea persistencia entre sesiones.
- Revisión completa del transporte MCP HTTP/SSE y parsing de comandos con espacios.
- Paginación y límites de la API; las colecciones siguen usando límites por página.
- Pruebas funcionales de todos los targets macOS Intel/Apple Silicon, Linux, Windows y WASM.
- Incorporación del motor Git local y su interfaz (etapa E1 y posteriores).
- `cargo fmt` puede informar de problemas de formato existentes en código anterior; no se debe forzar reformateo masivo sin revisar diferencias.

## Criterio de cierre

E0 sólo podrá etiquetarse como **VALIDADA** cuando se obtengan resultados correctos de compilación y tests, se revisen las regresiones manuales y quede documentada la compatibilidad por plataforma. No se integrará nada en `main` hasta esa verificación y una autorización explícita.
