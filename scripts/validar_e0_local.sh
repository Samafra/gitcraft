#!/usr/bin/env bash
# Validación local de la etapa E0 de GitCraft.
# No modifica ramas, no publica nada y no utiliza GitHub Actions.
# Ejecución: bash scripts/validar_e0_local.sh

set -u
set -o pipefail

# Situarnos en la carpeta del repositorio, independientemente del directorio actual.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

# Comprobar que tenemos Git y Cargo. Si faltan, detenerse sin alterar el proyecto.
for PROGRAM in git cargo; do
    if ! command -v "$PROGRAM" >/dev/null 2>&1; then
        echo "ERROR: falta $PROGRAM. No puede verificarse la etapa E0."
        exit 1
    fi
done

# Trabajar únicamente con develop, nunca con la rama estable main.
BRANCH="$(git branch --show-current)"
if [[ "$BRANCH" != "develop" ]]; then
    echo "ERROR: rama actual '$BRANCH'; se requiere develop."
    exit 1
fi

# No mezclar el resultado de las pruebas con modificaciones no confirmadas del usuario.
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: existen cambios sin confirmar. No se ejecutará la validación."
    exit 1
fi

# Guardar la referencia inicial para comprobar que el script no la modifica.
START_SHA="$(git rev-parse HEAD)"
echo "Validando GitCraft E0 sobre develop ($START_SHA)"
echo "Los comandos de Cargo pueden crear archivos de compilación dentro de target/."

# Continuar todos los controles aunque uno falle, y devolver resultado global al final.
FAILURES=0
run_check() {
    local NAME="$1"
    shift
    echo
    echo "===== $NAME ====="
    if "$@"; then
        echo "OK: $NAME"
    else
        echo "ERROR: $NAME"
        FAILURES=$((FAILURES + 1))
    fi
}

# Primero comprobar la compilación y ejecutar las pruebas automáticas del proyecto.
# --locked impide regenerar Cargo.lock silenciosamente.
run_check "Cargo check (plataforma local)" cargo check --locked
run_check "Cargo test (plataforma local)" cargo test --locked

# Comprobar formato. El resultado puede revelar problemas preexistentes, que hay que corregir.
if command -v cargo-fmt >/dev/null 2>&1 || cargo fmt --version >/dev/null 2>&1; then
    run_check "Rustfmt (revisión de estilo)" cargo fmt --all -- --check
else
    echo "ERROR: rustfmt no está instalado."
    FAILURES=$((FAILURES + 1))
fi

# Comprobar advertencias del analizador Clippy, cuando está disponible.
if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy (todos los targets locales)" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no está instalado."
    FAILURES=$((FAILURES + 1))
fi

# La compilación WASM es opcional en este script y sólo se intenta si el target está instalado.
# No se instalan herramientas ni se ejecuta un navegador automáticamente.
if command -v rustup >/dev/null 2>&1 && rustup target list --installed | grep -qx "wasm32-unknown-unknown"; then
    run_check "Cargo check (WebAssembly)" cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: target wasm32-unknown-unknown no instalado; no se valida WebAssembly."
fi

# Asegurar que ni el HEAD ni los archivos seguidos cambiaron mientras se comprobaba el código.
END_SHA="$(git rev-parse HEAD)"
if [[ "$START_SHA" != "$END_SHA" ]]; then
    echo "ERROR: el HEAD cambió durante la validación."
    FAILURES=$((FAILURES + 1))
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: aparecieron modificaciones en archivos seguidos."
    FAILURES=$((FAILURES + 1))
fi

# Un 0 indica que todas las comprobaciones ejecutadas finalizaron correctamente;
# la compatibilidad con otros sistemas necesita validaciones adicionales en cada plataforma.
echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: controles locales terminados sin errores; revisar aparte las pruebas manuales."
    exit 0
fi
echo "ERROR: $FAILURES controles fallaron o no estaban disponibles."
exit 1
