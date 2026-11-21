# GitCraft — Guía de pruebas excepcionales E0 (macOS)

**Rama de desarrollo:** `develop`  
**Rama estable protegida por la norma de trabajo:** `main` (no modificar).  
**Objetivo:** comprobar las situaciones que no quedan cubiertas por las siete pruebas básicas ya superadas.  
**Regla:** no realizar `Commit & Push`, merges ni modificaciones de archivos reales durante estas pruebas.

## 0. Preparar el repositorio local sin descartar datos

Abrir Terminal y situarse en el directorio donde esté instalada la copia de GitCraft. Ejemplo:

```bash
cd ~/Projects/gitcraft
git branch --show-current
git status --short
git fetch origin
git rev-parse --short HEAD
git rev-parse --short origin/develop
```

**Esperado:** rama `develop`, salida vacía de `git status --short`. Si hay archivos modificados, detenerse y revisarlos, **sin usar** `git reset`, `git restore` ni `git clean`.

Si los SHAs locales y remotos son distintos y no existe trabajo local pendiente, usar exclusivamente:

```bash
git pull --ff-only origin develop
```

Si no es posible avanzar por fast-forward, detenerse, recopilar `git status -sb` y `git log --oneline --max-count=3`, y revisar antes de actuar.

## A. Probar el aislamiento de las credenciales MCP con procesos ficticios

**Esta prueba se realiza sin token real y sin GitHub.** El propio test Rust crea una orden para ejecutar el programa local `env`. Antes de iniciar el proceso, simula que hay tres variables GitHub con valores claramente ficticios y aplica exactamente la misma política de GitCraft.

```bash
cargo test --locked mcp_environment_tests -- --nocapture
```

Comprobaciones que deben aparecer como correctas:
- `no_autorizado_no_hereda_credenciales_github`: el proceso hijo no recibe ninguna variable de autenticación GitHub.
- `autorizado_recibe_solo_token_ficticio`: el proceso hijo recibe exclusivamente el token ficticio autorizado en las variables previstas, y no recibe `GH_TOKEN`.

**Esperado:** 2 pruebas correctas, ninguna fallida. El nombre o número exacto de la suite podrá variar si se agregan nuevas pruebas. No copiar ni compartir variables reales del entorno.

**Importante:** esta prueba valida la política de entorno aplicada al proceso hijo. Sigue siendo necesaria la comprobación visual del consentimiento (ya realizada) y una eventual revisión futura del protocolo MCP completo.

## B. Probar respuestas antiguas al cambiar de repositorio o rama

1. Ejecutar `cargo run --locked` y conectar GitCraft con las credenciales habituales.
2. Abrir un repositorio **de solo lectura** en la pestaña Repositorios.
3. En Cambios y Ramas, cambiar rápidamente entre dos ramas existentes; comprobar que la selección mostrada coincide con los archivos mostrados.
4. Abrir un archivo, cambiar inmediatamente a otro y comprobar que el editor termina mostrando solamente el último archivo elegido.
5. Entrar en una carpeta, regresar, abrir otra y confirmar que nunca se mezclan resultados.
6. Seleccionar otro repositorio y repetir los pasos anteriores.
7. Desconectar mientras se muestra el indicador de carga y comprobar que no reaparece la sesión.

**Esperado:** sin errores inesperados, sin ficheros de una rama/repositorio anterior y sin resurrección de sesión tras desconectar. No editar ni guardar archivos.

Nota: si las respuestas de red llegan demasiado rápido, no se puede garantizar reproducir manualmente todas las carreras; los tests unitarios de los identificadores de petición complementan esta prueba.

## C. Crear una rama únicamente en un repositorio de pruebas

**Requisito previo:** disponer de un repositorio GitHub de pruebas distinto de `Samafra/gitcraft`, sin datos de producción. Esta guía no crea repositorios ni ramas automáticamente.

1. Dentro de GitCraft, seleccionar el repositorio **de pruebas**.
2. Entrar en Cambios y Ramas y comprobar el nombre del repositorio en la interfaz.
3. Escribir `prueba-e0-rama` en el campo de nueva rama; si ya existe, usar otro nombre de prueba.
4. Pulsar Crear Rama una sola vez.
5. Confirmar que la rama nueva aparece seleccionada.
6. Pulsar Actualizar ramas. Debe mantenerse seleccionada la nueva rama.
7. Consultar la rama en la web de GitHub del repositorio de pruebas y comprobar que existe.

**Esperado:** rama creada y seleccionada sin retorno automático a la rama predeterminada. No crear commits ni realizar merges. La rama queda disponible para inspección; no se borra automáticamente.

## D. Errores HTTP 403/429

Los tests de `src/github_manage.rs` generan una respuesta HTTP ficticia en `127.0.0.1`. Son suficientes para comprobar que errores 403 y 429 no se convierten en colecciones vacías.

```bash
cargo test --locked github_manage::tests -- --nocapture
```

**Esperado:** los tests `permission_denied_is_reported` y `rate_limit_is_reported` terminan correctamente. No se deben provocar errores reales de la cuenta de GitHub ni generar tráfico excesivo.

## E. Revalidación completa y estado limpio

Si todas las pruebas anteriores pasan, cerrar GitCraft y ejecutar:

```bash
bash scripts/validar_e0_local.sh 2>&1 | tee /tmp/gitcraft-e0-excepciones.log
git status -sb
git log -1 --oneline
```

**Esperado:** `OK: controles locales terminados sin errores` y `develop` sincronizada con `origin/develop`. Si apareciera un fallo de Rustfmt, inspeccionar su salida y **no** dar por validada E0; no modificar ni confirmar archivos sin revisar.

## F. Matriz de plataformas: no confundir compatibilidad prevista con verificada

| Plataforma | Estado documentado | Condición de validación |
| --- | --- | --- |
| macOS Intel | Compilación, tests y siete pruebas básicas superadas por el usuario; excepciones pendientes | Completar A–E |
| macOS Apple Silicon | Pendiente | Compilación y pruebas sobre equipo ARM |
| Linux x86_64 | Pendiente | Compilación y pruebas en Linux |
| Windows x64 | Pendiente | Compilación y pruebas en Windows |
| WebAssembly | Pendiente de evidencia específica | Compilar y probar en navegador; capacidades locales no equivalentes |

Ninguna plataforma no comprobada debe marcarse como validada.

## Entrega del resultado

Comunicar por separado:
- **A:** MCP ficticio OK / ERROR (con mensaje de error sin secretos).
- **B:** alternancia asíncrona y desconexión OK / incidencia.
- **C:** creación/actualización de rama en repositorio de pruebas OK / pendiente.
- **D:** 403 y 429 simulados OK / ERROR.
- **E:** script completo OK / ERROR y `git status -sb`.
- **F:** sistemas adicionales verificados, si los hubiera.

La etapa E1 (motor Git local) no se inicia hasta resolver los fallos de E0. No integrar en `main` sin autorización expresa y rollback comprobado.
