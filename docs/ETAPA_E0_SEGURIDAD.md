# GitCraft — E0: implementación de seguridad y estabilización

**Fecha:** 2026-11-21  
**Rama:** `develop` (exclusivamente)  
**Rollback previo:** commit `687579bc1715729b4ea7b649fda890ae79aa5cde` (informe y normas, antes de los cambios funcionales E0).  
**Referencia estable:** `main` en `c208de70e87c7eabf5e5e1fea5546aa1bae43ef4`.

## Estado de la etapa

**Implementación aplicada; validación de compilación y ejecución PENDIENTE.** El entorno que ha realizado esta revisión no contiene `cargo`, `rustc` ni `rustfmt`. No es correcto afirmar que los cambios compilan, que pasan los tests o que todas las plataformas funcionan hasta que se ejecuten las comprobaciones indicadas.

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

**Estas pruebas están escritas, no ejecutadas.** Los servidores de prueba HTTP sólo escuchan en `127.0.0.1`.

## Cómo validar en macOS/Linux

```bash
git clone https://github.com/Samafra/gitcraft.git
cd gitcraft
git switch develop
bash scripts/validar_e0_local.sh
```

Si ya existe una copia local limpia, no es necesario clonar de nuevo: actualizar `develop` por fast-forward y ejecutar el script.

El script evita ejecutar en `main`, no fuerza cambios, no ejecuta Actions ni publica nada. Cargo puede crear artefactos ignorados bajo `target/`. La comprobación de WebAssembly sólo se intenta si `wasm32-unknown-unknown` está instalado. Para Windows hay que ejecutar los comandos equivalentes con Cargo y hacer pruebas manuales específicas.

## Pruebas manuales pendientes

1. Inicio, autenticación GitHub y listado de repositorios; reiniciar la app y comprobar que solicita el PAT de nuevo.
2. Seleccionar repo y rama no predeterminada, actualizar ramas: no cambiar de rama.
3. Crear rama, verificar que queda seleccionada, abrir un archivo y editarlo en un repositorio de prueba.
4. Cambiar rápidamente entre repositorios/ramas/carpetas/archivos y comprobar que no se muestran resultados de otra selección.
5. Desconectar durante una petición, verificar que no reaparece una sesión antigua.
6. Elegir servidores MCP GitHub y no GitHub, verificar que no reciben PAT por defecto; autorizar manualmente y confirmar que la autorización se revoca al cambiar endpoint.
7. Simular permisos insuficientes o rate limiting y comprobar que GitHub muestra un error, no una colección vacía.

**No realizar pruebas de edición/merge sobre `main` ni sobre repositorios con datos de producción.** Usar repositorios temporales o de prueba.

## Verificación adicional realizada sobre el código fuente

- Se han releído los archivos modificados directamente desde `develop` y comprobado la correspondencia entre los tipos de `AsyncEvent`, sus emisores y sus receptores.
- Un comprobador estático de delimitadores (no un compilador Rust) no detectó llaves, paréntesis ni corchetes desparejados en los siete módulos Rust principales afectados.
- Se verificó en el código el uso de `#[serde(skip)]` para el PAT, la eliminación predeterminada de las variables de entorno del token en MCP y los nuevos tests de errores HTTP.
- Los tests del servidor HTTP simulado incluyen un límite de espera para no bloquear indefinidamente la validación.
- **No se ha ejecutado ninguna prueba Rust, no se ha compilado ningún target y no se ha probado la interfaz en ejecución**.

## Riesgos y asuntos pendientes fuera del alcance cerrado de E0

- Revisar e implementar almacenamiento seguro de credenciales nativo si se desea persistencia entre sesiones.
- Revisión completa del transporte MCP HTTP/SSE y parsing de comandos con espacios.
- Paginación y límites de la API; las colecciones siguen usando límites por página.
- Pruebas funcionales de todos los targets macOS Intel/Apple Silicon, Linux, Windows y WASM.
- Incorporación del motor Git local y su interfaz (etapa E1 y posteriores).
- `cargo fmt` puede informar de problemas de formato existentes en código anterior; no se debe forzar reformateo masivo sin revisar diferencias.

## Criterio de cierre

E0 sólo podrá etiquetarse como **VALIDADA** cuando se obtengan resultados correctos de compilación y tests, se revisen las regresiones manuales y quede documentada la compatibilidad por plataforma. No se integrará nada en `main` hasta esa verificación y una autorización explícita.
