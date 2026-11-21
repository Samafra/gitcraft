# GitCraft — E0: implementación de seguridad y estabilización

**Fecha:** 2026-11-21  
**Rama:** `develop` (exclusivamente)  
**Rollback previo:** commit `687579bc1715729b4ea7b649fda890ae79aa5cde` (informe y normas, antes de los cambios funcionales E0).  
**Referencia estable:** `main` en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`.

## Estado de la etapa

**E0 VALIDADA EN macOS INTEL dentro del alcance probado.** El usuario ha comunicado resultados correctos en compilación, tests, Rustfmt, Clippy, siete pruebas manuales básicas, navegación asíncrona y creación/actualización de una rama en un repositorio de pruebas independiente. Dos tests adicionales de aislamiento MCP con credenciales ficticias también terminaron correctamente. **E0 multiplataforma sigue parcialmente pendiente** (macOS Apple Silicon, Linux, Windows y WebAssembly). Esta validación no implica una auditoría integral de seguridad, ni autoriza modificar `main`. La revisión remota no ha ejecutado Cargo ni la interfaz gráfica.

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

**Alcance de esta evidencia:** controles automáticos correctos en el Mac del usuario. Las pruebas manuales de interfaz se documentan en las secciones siguientes. La evidencia de macOS Intel no acredita por sí sola compatibilidad macOS ARM, Linux, Windows o navegador WASM. El script sólo comprueba WebAssembly si el target está instalado; no consta el resultado específico de ese paso.

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

## Validación excepcional preparada — procesos MCP con credenciales ficticias

Se han incorporado dos tests de proceso real en `src/mcp_client.rs`. Ejecutan el programa local `env` con valores de prueba (`E0_DUMMY_...`), sin transmitir ni imprimir el token real, y verifican el aislamiento de `GITHUB_PERSONAL_ACCESS_TOKEN`, `GITHUB_TOKEN` y `GH_TOKEN`.

- `no_autorizado_no_hereda_credenciales_github`: un proceso no autorizado no debe recibir las variables.
- `autorizado_recibe_solo_token_ficticio`: un proceso autorizado recibe únicamente el token ficticio por las variables previstas.

Se ha extraído la configuración de variables de entorno a `configure_mcp_github_environment`, que utiliza el cliente real. **El usuario ejecutó las dos pruebas de aislamiento MCP correctamente en macOS** y comunicó literalmente: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s`. Estos resultados no forman parte de las siete pruebas manuales anteriores: son tests automatizados adicionales.

Los pasos completos para ejecutarlos, probar cambios rápidos de repositorio, crear una rama exclusivamente de prueba y controlar 403/429 están documentados en [Guía de pruebas excepcionales E0](PRUEBAS_EXCEPCIONALES_E0.md).

El usuario volvió a ejecutar `scripts/validar_e0_local.sh` después de corregirse en `develop` los tres detalles de Rustfmt del código MCP. Resultado comunicado: `OK: controles locales terminados sin errores; revisar aparte las pruebas manuales.` El commit de esa corrección es `a31e287321191fb3d4cd7e2d48ca2bc8e8854544`. Los tests fueron ejecutados localmente por el usuario; la revisión remota sólo verifica el código y los commits publicados.

## Evidencia de pruebas excepcionales macOS Intel — 21/11/2026

El usuario confirmó expresamente los siguientes resultados, **sin incidencias ni observaciones**:

- **Navegación asíncrona: OK.** Se verificaron los cambios entre repositorios, ramas, carpetas y archivos, así como el comportamiento de desconexión en la interfaz.
- **Creación de rama en repositorio de pruebas: OK.** Se creó una rama fuera de `Samafra/gitcraft` y se comprobó que permanecía seleccionada tras actualizarla.
- **Aislamiento de credenciales MCP: OK.** Dos tests de proceso local con tokens ficticios, 0 fallos.
- **Revalidación de Cargo: OK.** El script completo `scripts/validar_e0_local.sh` se ejecutó correctamente después de la corrección de Rustfmt.
- **Errores HTTP 403/429: cubiertos por pruebas automatizadas.** La ejecución completa de `cargo test` pasó; no se afirma haber reproducido esos errores en una sesión gráfica real.

### Compatibilidad por plataforma

| Plataforma | Resultado E0 |
| --- | --- |
| **macOS Intel (x86_64)** | **VALIDADA en el alcance comprobado**: Cargo + tests + formato + Clippy + navegación, autenticación, ramas y permisos MCP |
| macOS Apple Silicon (arm64) | Pendiente de compilación y pruebas en un equipo ARM |
| Linux | Pendiente de compilación y pruebas en Linux |
| Windows | Pendiente de compilación y pruebas en Windows |
| WebAssembly/navegador | Pendiente de evidencia específica de compilación y ejecución web |

**No deben presentarse las demás plataformas como verificadas.** No se modifica `main`; cualquier integración futura requerirá autorización explícita y un punto de recuperación.

## Verificación adicional realizada sobre el código fuente

- Se han releído los archivos modificados directamente desde `develop` y comprobado la correspondencia entre los tipos de `AsyncEvent`, sus emisores y sus receptores.
- Un comprobador estático de delimitadores (no un compilador Rust) no detectó llaves, paréntesis ni corchetes desparejados en los siete módulos Rust principales afectados.
- Se verificó en el código el uso de `#[serde(skip)]` para el PAT, la eliminación predeterminada de las variables de entorno del token en MCP y los nuevos tests de errores HTTP.
- Los tests del servidor HTTP simulado incluyen un límite de espera para no bloquear indefinidamente la validación.
- **La revisión remota no ha ejecutado Rust ni probado la aplicación gráficamente.** Sí se recibió el resultado satisfactorio de los controles locales del usuario en macOS. Las siete comprobaciones funcionales básicas y las dos excepcionales (navegación y creación de rama) fueron confirmadas posteriormente por el usuario. Siguen pendientes los otros targets.

## Riesgos y asuntos pendientes fuera del alcance cerrado de E0

- Revisar e implementar almacenamiento seguro de credenciales nativo si se desea persistencia entre sesiones.
- Revisión completa del transporte MCP HTTP/SSE y parsing de comandos con espacios.
- Paginación y límites de la API; las colecciones siguen usando límites por página.
- Pruebas funcionales de macOS Apple Silicon, Linux, Windows y WASM; macOS Intel está validado en el alcance documentado.
- Incorporación del motor Git local y su interfaz (etapa E1 y posteriores).
- `cargo fmt` puede informar de problemas de formato existentes en código anterior; no se debe forzar reformateo masivo sin revisar diferencias.

## Criterio de cierre

**Cierre E0 en macOS Intel: VALIDADO** con resultados de compilación, tests, formato, Clippy y pruebas manuales comunicados por el usuario. **Cierre E0 multiplataforma: PENDIENTE** de verificaciones en los otros targets, reflejados en la matriz. Este cierre parcial permite planificar la etapa E1 en `develop`, pero no constituye autorización para fusionar en `main`. Cualquier integración en `main` exige previamente verificar ambas ramas, crear un punto de rollback recuperable y obtener autorización explícita.
