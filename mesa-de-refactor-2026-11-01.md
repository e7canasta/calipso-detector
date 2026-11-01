# Mesa de refactor — el detector a tres semanas de v4

2026-11-01 · Sobre `main@7e7f443`. Formato: el de la
[mesa del proyecto 17](../calipso-roadmap/projects/17-escena-v3-en-vivo/docs/04-mesa-de-diseno.md)
y la [mesa R7](../calipso-docs/specs/r7/01-mesa-de-diseno.md). Material:
[arquitectura-c4.md](arquitectura-c4.md) y
[revision-2026-11-01.md](revision-2026-11-01.md).

Cada voz aplica las ideas publicadas de una escuela; no son citas. La pregunta
es una sola: **¿qué refactorizaríamos hoy para que el motor siga siendo fácil
de cambiar, sin tocar ninguna regla y sin cambiar un byte de lo publicado?**

> **Vara de la mesa.** Un refactor no cambia una celda de una tabla dorada, un
> evento publicado ni un `.feature` (DESIGN §10). Cada parche es chico, se
> puede bisecar y se verifica con `quick.sh`, las tablas y `SAME` sobre `clip1`,
> `dig1` y `galia` (como R2–R5 del proyecto 34). El motor sigue a **una muestra
> por segundo**: la cadencia es del sistema, no del motor.

---

## 1. Dónde duele hoy

El motor no tiene un problema de diseño; tiene **puntos donde se acumula el
crecimiento**. Desde el 10-11 el `src` del núcleo pasó de 7 000 a 12 550
líneas, y casi todo entró por los mismos cuatro archivos:

| Archivo | Líneas | Por qué crece |
| --- | --- | --- |
| `attention.rs` | 852 | Cada mirada nueva es una función más acá (foco, refuerzo, caras, cuerpo dudoso, pose, profundidad, calibración, verificar, ráfaga) |
| `evidence.rs` | 681 | Cada sentido nuevo trae su admisión |
| `cycle/sequence.rs` | 634 | Cada mirada nueva es un `Reached` y una `Continuation` más en `Course::next` |
| `detector/src/config.rs` | 1 750 (630 de `impl Config`, 580 de pruebas) | Cada capacidad nueva es una sección del perfil y su traducción a `Policy` |

Ninguno rompe una regla. Los cuatro hacen lo mismo: **un archivo por paso del
lazo, que suma una pieza por cada sentido**. El refactor que piden es el mismo
en los cuatro: pasar de «un archivo por paso» a «un módulo por paso, un
archivo por pregunta».

## 2. Las voces

### Martin Fowler — refactorizar a pasos chicos

- **Split Phase en `Course::next`.** Los doce `Reached` son en realidad tres
  fases con frontera clara: *percibir* (`Focused` → `Evidence`), *creer y
  enriquecer* (`Belief` → `Body`) y *cerrar* (`Refined` → `Final`). Cada fase
  recibe lo que dejó la anterior y no mira hacia atrás. Partir `next` en
  `perceive`, `enrich` y `close`, cada uno con su enum, deja el orden en un solo
  lugar (regla 11) pero legible de a un tercio.
- **Move Function con `attention`.** Un módulo `attention/` con un archivo por
  pregunta (`lens`, `reinforce`, `faces`, `body_pose`, `enrich`, `depth`,
  `calibrate`, `verify`) y `memory.rs`. Es mover, no rediseñar: las firmas no
  cambian.
- **No tocar lo que está bien.** El protocolo `drive`/`cancel`, `Monitor`,
  `Room`, las máquinas por capa y el contrato no se tocan. Refactorizar lo
  sano es cambiar por cambiar.

### Kent Beck — diseño simple

- **La regla de tres ya se cumplió con las tablas doradas.** Cinco tablas
  (`layer-0`, `parts`, `parts-reading`, `bed-posture`, `second-level`)
  repiten la misma prueba: generar la tabla, compararla con el archivo,
  imprimir la nueva si cambió. Una función de prueba común
  (`assert_golden_table(nombre, generada, archivo)`) saca la duplicación sin
  agregar abstracción al código de producción.
- **No hay segunda implementación de `Decider`.** Las tres máquinas tienen la
  misma forma, pero un trait `Decider` no tiene un segundo uso real: cada una
  se llama desde un solo lugar. Regla 18: **no**.

### Vaughn Vernon — contextos y lenguaje

- **`cycle/engine.rs` está en el contexto equivocado.** Son las operaciones
  del motor sobre sus memorias, no la secuencia. Vive bajo `cycle` porque
  `Engine` está declarado en `cycle.rs`. Mover `Engine` a su propio módulo
  (`engine/`) hace que la regla 11 se lea en el árbol de archivos: `cycle`
  secuencia, `engine` cambia.
- **La verificación es un subdominio.** Hoy la mitad está en `verification`
  (qué se preguntó, qué se debe) y la otra mitad en `attention` (`verify`,
  `burst`, `burst_ready`). Juntarla en `attention/verify.rs` (qué mirar) y
  `verification` (qué se sabe) deja una frontera que se nombra igual en el
  white book III.
- **Un solo glosario.** El `DESIGN.md` de la raíz es una copia vieja del de la
  crate. Borrarlo (o enlazarlo): el lenguaje ubicuo tiene una sola casa
  (regla 20).

### El kernel — mecanismo, interfaces, parches

- **Kconfig y defconfig.** El kernel compila cada `defconfig` en CI: una
  configuración que no arranca no llega a `main`. Equivalente acá: **cada
  `config/*.toml` pasa `detector --check`** en `quick.sh`, y cada perfil de
  producción tiene una escena golden que lo recorre. Hoy `config.rs` valida
  campos sueltos (`fraction`, `positive`), no combinaciones.
- **Partir `config.rs` como se parte un driver.** Un archivo por sección del
  perfil (`stream`, `senses`, `policy`, `map`, `verify`, `geometry`), cada uno
  con su `impl From<…Config> for …Policy`. La traducción del perfil a la
  política queda al lado de su sección, como cada subsistema del kernel tiene
  su `Kconfig`.
- **`uapi`: ahora sí hay disparador.** La mesa 17 dijo «no por ahora: sin
  consumidor en Rust fuera del repo». Hoy hay dos (`rerun-bridge`, `shark`) y
  dependen del núcleo entero para leer el contrato. Separar `contract` en una
  crate de tipos del cable (`scene-contract`) es un movimiento mecánico, como
  la división de `include/uapi` de 2012: mismos bytes, mismo schema, mismos
  snapshots. Va **después** de los demás parches, porque mueve más archivos.
- **Una segunda puerta documentada no es un bug.** `Monitor::verified` entra
  fuera de `Input`. Como la grabación está fuera del detector, no hay registro
  que se rompa. Se documenta en DESIGN §3 y no se toca.

### Rust — el diseño en tipos

- **Una fase, un enum.** Partir `Reached` en tres enums (uno por fase) hace que
  el compilador impida saltar de *percibir* a *cerrar*: hoy lo impide la
  disciplina de `next`.
- **Visibilidad por módulo.** Al pasar `attention` y `evidence` a carpetas,
  lo que solo usa un archivo queda `pub(super)` y no `pub(crate)`.
  `unreachable_pub` ya lo vigila hacia afuera; esto lo extiende hacia adentro.
- **Sin macros ni genéricos nuevos.** Ningún refactor de esta mesa necesita un
  `macro_rules!` ni un trait nuevo en producción.

### David Parnas — qué secreto esconde cada módulo

- La prueba de un módulo es **qué decisión esconde**. `room` esconde la
  geometría (y por eso tiene guardián). `attention` hoy esconde nueve
  decisiones; después del refactor esconde una por archivo. `config.rs`
  esconde «cómo se escribe un perfil» y «cómo se arma la política»: son dos
  secretos y deberían ser dos módulos.
- **«El espacio tiene un solo dueño» es la regla 23.** Ya tiene guardián
  (`space-outside-room` y `room-reads-a-layer`); le falta número y ADR. Que
  la cabeza haya pasado de `room` a `refine` (`3e9152a`) es esta misma idea
  aplicada: `room` esconde la geometría y nada más.

## 3. La pizarra

| # | Refactor | Voz | Regla que protege | Riesgo | Verifica |
| --- | --- | --- | --- | --- | --- |
| F-1 | Borrar el `DESIGN.md` de la raíz (o un enlace al de la crate) | Vernon | 20 | nulo | — |
| F-2 | Regla 23 en `RULES.md`, con ADR | Parnas | 23 | nulo | `check-guardians.sh` |
| F-3 | Prueba común de tablas doradas | Beck | P2 | nulo (solo tests) | las cinco tablas |
| F-4 | `detector --check` sobre cada `config/*.toml` en `quick.sh` | kernel | 13, 18 | nulo | `quick.sh` |
| F-5 | `Engine` a su módulo; `cycle/engine.rs` → `engine/` | Vernon | 11 | bajo | `quick.sh`, tablas, `SAME` |
| F-6 | `attention/` como carpeta, un archivo por pregunta; `verify` y `burst` juntos | Fowler, Vernon | 12 | bajo | `quick.sh`, tablas, `SAME` |
| F-7 | `evidence/` como carpeta, un archivo por sentido | Fowler | 6 | bajo | ídem |
| F-8 | `Course::next` en tres fases con su enum | Fowler, Rust | 10, 11 | medio | ídem + los 335 escenarios |
| F-9 | `config.rs` en un módulo por sección, con su `From` | kernel, Parnas | 13 | medio | `quick.sh`, F-4 |
| F-10 | `scene-contract` como crate de tipos del cable | kernel | 13, C1 | medio (mueve archivos entre crates y repos) | snapshots del schema, `cargo check` de bridge y shark |

## 4. Lo que no entra

| Tema | Por qué |
| --- | --- |
| Cadencia variable (acelerar o frenar el reloj según el estado) | Agrega un lazo de control al motor para ganar un segundo. Con `dwell` en segundos de fuente (regla 9) y ya distinto por estado (`dwell_getting_out_secs = 1.0`), la latencia se ajusta en el perfil. Si algún día el sistema entero pasa a 0,5 s, el motor no cambia: el `dwell` sigue en segundos |
| Un trait `Decider` común | Sin segundo uso real (regla 18) |
| Statechart o guardas como datos (el borrador de orbit) | El mecanismo no es configurable (DESIGN §4); las tablas doradas ya son el dato que se versiona |
| `Input::Verified` | La grabación está fuera del detector; la segunda puerta no rompe nada. Se documenta |
| Incertidumbre publicada por capa | Es función nueva y cambia el contrato: va a una mesa de diseño, no a una de refactor |
| Análisis de peligros | No es la capa del detector: es de quien recibe los mensajes y avisa |

## 5. La secuencia

```text
 Ola 0   F-1  F-2  F-3  F-4                 (una tarde, sin tocar producción)
 Ola 1   F-5 → F-6 → F-7                     (mover, un parche cada uno)
 Ola 2   F-8                                 (el único que cambia la forma de la vuelta)
 Ola 3   F-9                                 (cáscara)
 Ola 4   F-10                                (cuando haya ventana: toca otros repos)
```

Cada ola entra a `main` con el gate completo (regla 17) y `SAME` en los tres
clips. Si un parche cambia una celda, un evento o un escenario, **no es un
refactor**: se revierte y se discute.

## 6. Lo hecho (2026-11-01)

En `main` de calipso-detector, un commit por parche, cada uno con
`quick.sh` en verde (las tablas doradas, 261 escenarios del núcleo y 78 de
la cáscara), coordinado con las otras sesiones (bridges y claqueta).

| # | Commit | Nota |
| --- | --- | --- |
| F-1 | `57058e9` | El `DESIGN.md` de la raíz queda como puntero, no se borra: el proyecto 26 del roadmap lo enlaza |
| F-2 | `4069f06` | Con su ADR en calipso-docs (`40fb3e9`) |
| F-3 | `b7b31c7` | `golden::assert_table` y `golden::name`, solo de pruebas. Las cinco tablas muestran ahora las filas que cambiaron |
| F-4 | `970a404` | Es una prueba del detector, no `detector --check`: no necesita el binario. Se comprobó que falla con un perfil roto. `quick.sh` mapea `config/` al detector |
| F-5 | **descartado** | `Course` lee los campos privados de `Engine`. Sacarlo de `cycle` obligaría a hacerlos `pub(crate)`: menos ocultamiento, no más (Parnas). `cycle.rs` declara a propósito el estado de la vuelta y sus hijos lo implementan |
| F-6 | `93f5275` | `attention/`: `primary`, `faces`, `body`, `depth`, `verify`, `memory`, `ledger`. Las rutas públicas no cambian; el schema tampoco (toma el nombre del tipo, no la ruta) |
| F-7 | `5bff44e` | `evidence/`: `bodies`, `faces`, `pose`, `depth`, cada uno con sus pruebas |
| F-8, F-9, F-10 | pendientes | F-8 cambia la forma de la vuelta: merece su propia sesión. F-10 toca los bridges: coordinarlo con su sesión |

El gate completo (`check.sh`) corrió una vez sobre la serie, en verde (formato, lints, los árboles de dependencias, los guardianes del dominio y del espacio, todas las pruebas, licencias).
