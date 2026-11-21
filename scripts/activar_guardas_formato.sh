#!/bin/sh
# Activa las guardas sólo en ESTE repositorio local y sólo con consentimiento.
# No instala hooks globales ni sobrescribe un core.hooksPath personalizado.
set -eu

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "ERROR: no se encuentra la raíz Git." >&2
    exit 1
}
cd "$ROOT"

if [ "$(git branch --show-current)" != "develop" ]; then
    echo "ERROR: ejecutar la activación únicamente desde develop." >&2
    exit 1
fi

PREVIOUS="$(git config --local --get core.hooksPath || true)"
if [ -n "$PREVIOUS" ] && [ "$PREVIOUS" != ".githooks" ]; then
    echo "No se ha modificado la configuración Git." >&2
    echo "Ya existe un directorio de hooks personalizado: $PREVIOUS" >&2
    echo "Revisar su integración manualmente antes de cambiarlo." >&2
    exit 1
fi

git config --local core.hooksPath .githooks
echo "Guardas locales activadas en GitCraft: pre-commit y pre-push."
echo "No se han modificado archivos ni ramas."
