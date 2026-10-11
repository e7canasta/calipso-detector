#!/usr/bin/env bash
# Runs only the @wip scenarios (specs in progress). It does not block
# integration: it is the red the pair is turning green.
set -euo pipefail
cd "$(dirname "$0")/.."
CALIPSO_WIP=1 cargo test -q -p scene-core --test bdd -- "$@"
