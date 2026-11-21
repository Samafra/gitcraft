#!/usr/bin/env bash
# GitCraft E1 — Validación local, sin modificar ramas ni archivos del usuario.
#
# Este script verifica el motor Git local de lectura, además de la compilación
# y las regresiones previas. Sólo trabaja en develop y no usa GitHub Actions.
# Las pruebas Rust crean repositorios ficticios dentro del directorio temporal.

set -u
set -o pipefail

# Accedemos a la raíz de GitCraft tomando como referencia este archivo.
ROOT="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

for PROGRAM in git cargo; do
    if ! command -v "$PROGRAM" >/dev/null 2>&1; then
        echo "ERROR: falta $PROGRAM. La etapa E1 no puede validarse."
        exit 1
    fi
done

# Nunca ejecutar una verificación que pudiera mezclarse con trabajo sin guardar.
BRANCH="$(git branch --show-current)"
if [[ "$BRANCH" != "develop" ]]; then
    echo "ERROR: estás en '$BRANCH'; se requiere develop."
    exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: existen cambios sin confirmar. No se ejecutará la validación."
    exit 1
fi

START_SHA="$(git rev-parse HEAD)"
echo "E1: validación del commit $START_SHA en develop"
echo "Git disponible: $(git --version)"
echo "Las pruebas crean únicamente carpetas temporales y Cargo puede usar target/."

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

# El formato se verifica sin escribir modificaciones en el árbol de trabajo.
run_check "Rustfmt E1" cargo fmt --all -- --check
run_check "Compilación nativa E1" cargo check --locked
run_check "Tests de repositorios Git temporales E1" \
    cargo test --locked local_git::tests -- --nocapture
run_check "Todos los tests y regresiones E0" cargo test --locked

# El análisis estático se comprueba en todos los targets de la máquina.
if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy E1" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no está disponible."
    FAILURES=$((FAILURES + 1))
fi

# La versión Web no debe compilar el motor nativo: se verifica si existe el target.
if command -v rustup >/dev/null 2>&1 \
    && rustup target list --installed | grep -qx "wasm32-unknown-unknown"; then
    run_check "Compilación WebAssembly sin Git local nativo" \
        cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: comprobar WebAssembly en un entorno con target instalado."
fi

# Comprobamos que ninguna operación ha modificado los archivos seguidos.
if [[ "$START_SHA" != "$(git rev-parse HEAD)" ]]; then
    echo "ERROR: el commit HEAD cambió durante las pruebas."
    FAILURES=$((FAILURES + 1))
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: hay archivos seguidos modificados después de las pruebas."
    FAILURES=$((FAILURES + 1))
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: validación automática E1 local completada; faltan pruebas visuales."
    exit 0
fi
echo "ERROR: $FAILURES verificaciones de E1 fallaron o no estaban disponibles."
exit 1
