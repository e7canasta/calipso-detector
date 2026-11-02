# AGENTS.md — calipso-detector

Este repositorio es el centro de Calipso. Un agente que entra sin contexto
lee en este orden:

1. [`docs/README.md`](docs/README.md): el hub y el canon de cada pregunta.
2. [`RULES.md`](RULES.md): los invariantes. Cada regla cita su guardián.
3. El `DESIGN.md` de la crate que vas a tocar.

## Reglas de trabajo

- Trabajar en el checkout, con su `target/` caliente. Sin worktrees ni copias del repo.
- Antes de editar, `git status`. Puede haber otra sesión con archivos modificados;
  no los pises ni los commitees sin preguntar al dueño del cambio.
- Especificación antes que código: un escenario `.feature` en rojo, después el cambio.
- Verificar al tamaño del cambio (regla 17):
  - por cambio: `cargo test -p <crate> --test bdd -- --name '…'`;
  - por commit: `scripts/quick.sh`;
  - por versión: `scripts/check.sh`.
- Compilar una sola vez por revisión (regla 22); no volver a compilar lo ya verificado.
- Una decisión que cambia una regla se registra como ADR nuevo en
  `calipso-docs/adr/`. No se reescribe la regla en silencio.

## Estado y superficies

- Estado del trabajo: `calipso-roadmap`, no este repo.
- Contratos publicados: `contract/`. El detector publica solo `scene.v5` y `events.v5`. `v4` se retiró con el corte ([ADR](../calipso-docs/adr/2026-11-02-retirar-v4.md)) y `v3` antes ([ADR](../calipso-docs/adr/2026-10-11-retirar-v3.md)).
- Lo que no está en este repo: ver `docs/README.md`, sección de consumidores.

## Pendientes conocidos

- `RUN.md` tiene rutas de la máquina de desarrollo. No copiar sus rutas a otros documentos.
- No hay procedimiento repetible de puesta en marcha en sitio. Ver `docs/README.md`.
