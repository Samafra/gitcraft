#!/usr/bin/env bash
# GitCraft E3-B: creación local de ramas y etiquetas con confirmación.
#
# No ejecuta GitHub Actions y no crea refs en el repositorio real de GitCraft.
# Los tests Rust preparan repositorios temporales independientes.

set -u
set -o pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

for PROGRAM in git cargo; do
    if ! command -v "$PROGRAM" >/dev/null 2>&1; then
        echo "ERROR: no está instalado $PROGRAM."
        exit 1
    fi
done

if [[ "$(git branch --show-current)" != "develop" ]]; then
    echo "ERROR: las pruebas E3-B sólo se ejecutan sobre develop."
    exit 1
fi
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "ERROR: existen archivos modificados o no seguidos; revisarlos antes de validar."
    exit 1
fi

START_HEAD="$(git rev-parse HEAD)"
echo "Validación GitCraft E3-B del commit $START_HEAD"
echo "Los tests de escritura utilizan exclusivamente repositorios temporales."

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

# --check verifica las diferencias de formato sin editar código fuente.
run_check "Rustfmt E3-B" cargo fmt --all -- --check
run_check "Compilación E3-B" cargo check --locked
run_check "Tests de referencias locales E3-B" \
    cargo test --locked local_git_refs::tests -- --nocapture
run_check "Regresiones E0, E1, E2, E3-A y E3-B" cargo test --locked

if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy E3-B" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no está disponible."
    FAILURES=$((FAILURES + 1))
fi

# Los módulos Git nativos se excluyen del target WebAssembly.
if command -v rustup >/dev/null 2>&1 \
    && rustup target list --installed | grep -qx "wasm32-unknown-unknown"; then
    run_check "Compilación WebAssembly" \
        cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: no está instalado el target wasm32-unknown-unknown."
fi

# El script sólo lee el repositorio real; HEAD y sus cambios se conservan.
if [[ "$(git rev-parse HEAD)" != "$START_HEAD" ]]; then
    echo "ERROR: HEAD ha cambiado durante la validación."
    FAILURES=$((FAILURES + 1))
fi
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "ERROR: el árbol de trabajo de GitCraft ha dejado de estar limpio."
    FAILURES=$((FAILURES + 1))
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: controles automáticos E3-B completados; faltan pruebas visuales."
    exit 0
fi
echo "ERROR: $FAILURES verificaciones E3-B fallaron."
exit 1
