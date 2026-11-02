# Hub de documentación — calipso-detector

Punto de entrada para agentes y personas. Este repositorio es el centro de la
solución Calipso: su código, sus reglas y sus tests mandan sobre cualquier
documento. Si un texto contradice el código, el texto está mal.

Los repositorios hermanos están en `../..`:

| Repo | Qué es | Entrada |
| --- | --- | --- |
| `calipso-detector` (este) | El motor por cámara: ingesta, sentidos, núcleo, publicación | este hub |
| `calipso-docs` | Libros, white book, ADR, specs históricas, guías | [`../../calipso-docs/README.md`](../../calipso-docs/README.md) |
| `calipso-roadmap` | Proyectos, estado y handoffs | [`../../calipso-roadmap/README.md`](../../calipso-roadmap/README.md) |
| `calipso-rerun-bridge`, `calipso-perception-bridge`, `calipso-golden`, `calipso-verifier` | Consumidores y herramientas | cada uno tiene su README |
| `calipso-claqueta` | Calibración y aceptación en escena | su README |
| `calipso-dist` | Distribución y perfiles de sitio | su README |
| `calipso-shark` | Repo citado por `RUN.md` y las ADR; no está clonado en esta máquina | — |

## Canon: qué manda sobre qué

| Pregunta | Fuente que manda | Notas |
| --- | --- | --- |
| ¿Qué invariantes no se negocian? | [`../RULES.md`](../RULES.md) | Cada regla tiene su guardián en `scripts/` o `features/` |
| ¿Cómo se comporta el núcleo? | [`../crates/scene-core/DESIGN.md`](../crates/scene-core/DESIGN.md) y `features/` | Los `.feature` son ejecutables |
| ¿Cómo se configura y corre un sitio? | [`onboarding.md`](onboarding.md) (procedimientos A–E) y [`../RUN.md`](../RUN.md) | `RUN.md` tiene rutas de la máquina de desarrollo; ver «Pendientes» |
| ¿Qué contrato sale a NATS? | [`../contract/`](../contract) | JSON schema; el detector publica `scene.v4` y `events.v4` siempre, y `scene.v5` y `events.v5` si el perfil los lista en `[publish] versions` |
| ¿Cómo se verifica un cambio? | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) y la regla 17 de `RULES.md` | `scripts/quick.sh` por commit, `scripts/check.sh` por versión |
| ¿Por qué es así? | [`../../calipso-docs/adr/`](../../calipso-docs/adr) | Las ADR no se reescriben; se reemplazan |
| ¿Cómo se diseña el sistema completo? | [`../arquitectura-c4.md`](../arquitectura-c4.md) (canon) y [`../../calipso-docs/big-picture.md`](../../calipso-docs/big-picture.md) (canon de la visión general) | — |
| ¿Qué sigue? | `calipso-roadmap` | Los proyectos activos y sus handoffs |

## Cómo se trabaja acá

1. Leer `RULES.md` y el `AGENTS.md` del repo.
2. Verificar en la crate que cambió (`cargo test -p <crate> --test bdd`).
3. Antes de commit, `scripts/quick.sh`. Antes de versión, `scripts/check.sh`.
4. Una decisión nueva que cambie una regla: un ADR nuevo en `calipso-docs/adr/`, no una edición silenciosa.

## En curso

- Proyecto 38, costuras del núcleo: [frentes](../../calipso-roadmap/archivo/proyectos/38-costuras-del-nucleo/frentes.md).
  Regla 24 de [`RULES.md`](../RULES.md) («el negocio se escribe en el código»),
  con su guardián en `crates/scene-core/tests/guardians.rs` y su
  [ADR](../../calipso-docs/adr/2026-11-02-el-negocio-se-escribe-en-el-codigo.md).

## Pendientes de documentación

- `RUN.md` depende de rutas de la máquina de desarrollo (`/home/visiona/...`). Hay que generalizarlo o moverlo a una plantilla por sitio.
- La puesta en marcha en un sitio real (cámara, red, NATS, perfil) no tiene un procedimiento repetible. Hace falta uno con su verificación.
- Las salidas de `onboarding.md` son de 2026-10-11; volver a correrlas antes de citarlas.
