# Cambios de forma del contrato v5

Estado: entregado, sin commit (2026-11-04). Dueño: master.

Acta de cada cambio de forma del wire v5 que se hace en sitio (el dueño
autoriza evolucionar v5: los tipos y el schema cambian en su lugar; el motor
publica lo mismo). Cada entrada dice qué cambió (JSON y API Rust), por qué,
qué le pasa a un consumidor que publicó con el formato viejo y qué debe hacer
un consumidor Rust viejo. Los cambios de frentes posteriores (F1c en
adelante) se agregan como entradas nuevas al final, una por frente.

Regla común de las entradas de F1b: **ningún valor publicado cambia**. Los
mensajes de `contract/examples/` siguen byte a byte (se verifican contra su
snapshot) y validan contra el schema regenerado. Lo que cambia es el schema
(`contract/schema/*.json`, regla 13) y la API Rust de `scene-contract`.

## F1b — invariantes en los tipos y vocabulario único

### 1. Los constructores de `Envelope` imponen la convención

- **JSON**: los mensajes no cambian. El `description` del `Envelope` en el
  schema documenta la convención (texto no normativo).
- **API Rust**: `v5::Envelope` es `#[non_exhaustive]` y se construye solo con
  `v5::Envelope::event(source, profile, seq, observed_at, published_at_ms,
  event_id, body)` y `v5::Envelope::scene(source, profile, seq, observed_at,
  published_at_ms, timing, last_event_seq, body)`. La literal
  `v5::Envelope { … }` deja de compilar fuera de `scene-contract`. Los campos
  siguen siendo públicos para leer.
- **Por qué**: la convención (un evento solo lleva `event_id`; una escena solo
  lleva `timing` y `last_event_seq`; cada mensaje nombra su `schema`) estaba
  escrita en el emisor y en cada consumidor que armaba mensajes. Ahora vive en
  el tipo: construir un `Envelope<Event>` con `timing` es ilegal fuera del
  crate (y los constructores no lo permiten dentro de él); lo único que lo
  conseguiría sería forjar JSON y deserializar.
- **Consumidor con el formato viejo**: lee igual; el schema solo crece texto.
- **Consumidor Rust viejo**: reemplazar cada literal `v5::Envelope { … }` por
  los constructores. El código que armaba el sobre a mano ya no nombra
  `SchemaId`: cada constructor escribe el que corresponde.

### 2. `Event::at` es el único «¿en qué muestra ocurrió?»

- **JSON**: sin cambios.
- **API Rust**: nuevo `v5::Event::at(&self) -> Option<SampleRef>`. `None` solo
  para un cambio de recepción, que no observa muestra (cambio 2 del ADR v5).
- **Por qué**: el match por variante que contestaba esa pregunta estaba
  copiado en el emisor (`detector/src/publish.rs`) y en los ejemplos
  (`scene-core/tests/contract_examples.rs`). Es forma del cable: se declara una
  vez, junto al `Event`, y el `observed_at` del sobre de un evento es ese
  valor.
- **Consumidor con el formato viejo**: sin efecto.
- **Consumidor Rust viejo**: si reimplementaba el match sobre las variantes,
  que pase a `event.at()`.

### 3. Un solo `Span`; `Counts` y `Fv` son el mismo par

- **JSON**: sin cambios de valores ni de nombres (`"span"`, `"fv"`,
  `"counts"` y sus campos, tal cual). En `calipso.events.v5.json` desaparece
  el `$defs/Span2`: había dos `Span` porque el tipo estaba declarado dos
  veces; `PartsSeen.span` pasa a apuntar a `$defs/Span`, cuya forma
  (`first`, `secs`) no cambia. Los `$defs` `Counts` (escena) y `Fv`
  (eventos) conservan nombre, forma y descripción.
- **API Rust**: `v5::Span` pasa a ser el `Span` de la raíz
  (`scene_contract::Span`) re-exportado: un solo tipo declarado. `Counts` y
  `Fv` siguen siendo dos nombres —cada uno con el suyo en el JSON: `"counts"`
  y `"fv"`— pero son el mismo par `floor`/`bed`, con la equivalencia
  documentada en los `From<Fv> for Counts` y `From<Counts> for Fv`.
- **Por qué**: cada concepto del vocabulario se declara una vez. El segundo
  `Span` era idéntico al de la raíz (su nota decía que existía «para que
  `Seen` nombre el `Fv` de v5», pero `Span` no menciona `Fv`). En vez de
  fusionar `Counts` y `Fv` en un tipo (que renombraría un `$defs` publicado),
  se dejó la equivalencia por escrito, que es lo que la tabla 0 dice: FV es
  el par de las zonas.
- **Consumidor con el formato viejo**: lee igual.
- **Consumidor Rust viejo**: `v5::Span` sigue existiendo y es el mismo tipo
  que `scene_contract::Span` (los dos paths nombran un tipo). Las conversiones
  a mano entre `Counts` y `Fv` pueden ser ahora los `From`.

### 4. `Cell` en las tres celdas; el DSL de `Parts` con patrón

- **JSON**: no cambian los valores. El schema:
  - `Parts.last_cell`, `PartsChange.cell` y `BedPostureChange.cell` pasan de
    `string` sueltos a `$ref: Cell` (los mismos valores de siempre, escritos
    igual).
  - `Cell` (el mismo objeto en ambos schema) documenta las tres notaciones de
    fila y su `pattern` las acepta: la de la capa 0 (`before:seen:after`,
    cifras o tuplas: `210:00:111`), la de las partes (`places | trama |
    places`) y la de la postura en cama (`estado | visto | estado`). Antes
    solo aceptaba la de la capa 0. Cuál de las tres corresponde a cada campo
    queda en el `description` del campo.
  - El DSL de las partes, que era `string` sin forma declarada, lleva
    `pattern` (schemars `regex`, el mismo mecanismo que `Cell` ya usaba):
    `Parts.state` (los lugares con HELD), `Parts.seen` y `PartsSeen.parts` (la
    trama de la muestra, con `-` donde no se miró) y `PartsPending.places` /
    `PartsBefore.places` / `PartsAfter.places` (los lugares sin HELD). Los
    patrones son fieles a la gramática real, la de
    `scene-core/src/project/parts.rs` (7 · 3 · 3 posiciones fijas: `cfhsaip` ·
    `fhs` · `BFK`, dos bits por posición): los 621 valores publicados en
    `contract/examples/` y los del corpus de compatibilidad validan contra
    ellos.
- **API Rust**: `Parts.last_cell` pasa de `Option<String>` a `Option<Cell>`;
  `PartsChange.cell` y `BedPostureChange.cell`, de `String` a `Cell` (newtype
  transparente sobre `String`). `Cell::as_str()` sigue igual y `Cell` compara
  con el `String` que lo escribió.
- **Por qué**: «celda» es un concepto del vocabulario (la clave natural de una
  fila de una tabla) y se declara una vez, en `Cell`, igual que la notación de
  la trama se declara una vez en sus patrones. El consumidor deja de
  re-implementar la forma del cable: el schema la declara y la valida.
- **Consumidor con el formato viejo**: lee igual —los patrones solo agregan
  forma a lo que ya se publicaba—. Una salvedad honesta: como `Cell` es un
  solo tipo para las tres tablas, el `pattern` de un campo `cell` concreto ya
  no descarta las notaciones de las otras tablas (el `description` del campo
  dice de qué tabla es la fila); antes el campo de la capa 0 era el único con
  `pattern` y los otros dos no tenían ninguno.
- **Consumidor Rust viejo**: los campos `cell`/`last_cell` pasan de `String` a
  `Cell`; se leen con `.as_str()` y se construyen con `Cell(…)` o con
  `scene_core::cell_of(...)`, como siempre.

## F1c — consistencia del wire y del lenguaje del bucle

Regla común de las entradas de F1c: **ningún valor publicado cambia**; los
mensajes de `contract/examples/` siguen byte a byte y los snapshots de
`contract/schema/*.json` ni se regeneraron (el movimiento de doc-strings fue
literal). Lo que cambia es la API Rust de `scene-contract`, su forma de
organizarse y los textos que la cuentan.

### 1. Una sola política de superficie pública, con guardián (decisión B)

**Medición, antes de decidir.** Un `#[non_exhaustive]` generalizado a todos
los enums publicados del wire (y a `UnitError`) exigiría brazos `_` en **9
`match` del workspace**, todos dentro de `scene-core`:
`belief/bed_posture/machine.rs` (`dwell_secs`, sobre `BedPostureState`),
`belief/bed_posture/rules.rs` (`decide`, sobre `BedPostureState`),
`body.rs` (`bed_site_at`, sobre `BedSite`), `room/shadow.rs` (sobre `Zone`) y
`tests/published.rs` (la cinta dorada: `FactBody`, `Event`, `DerivedBody` y
dos `let` sobre `MapBody`/`SensorBody`). Ninguno en `router.rs` ni en `sense`
(las guardianas que deniegan wildcards), pero cuatro de esos nueve son
matches del core exhaustivos a propósito, donde el compilador señala cada
decisión del dominio.

**Decisión: B — dos clases, con guardián.** La regla 13 manda sobre la guía:
un cambio incompatible del contrato publicado es versión mayor con ADR. Eso es
exactamente el criterio de las dos clases:

- **abierto** — status, razón, fallo o etiqueta (quién/cuál/cómo) de una
  observación, una mirada o la recepción, que puede crecer dentro de v5 sin
  cambiar lo que ya se publica: `#[non_exhaustive]`, para que el consumidor
  Rust tolere lo desconocido. Son `Provenance`, `SensorState`, `LookKind`,
  `Failure`, `RepublishReason`, `SkipReason`, `BodySource`, `RejectReason`,
  `Cause`, `Verifier`, `HeadEvidence`, `Geometry`, `LookResult`,
  `LookOutcome`, `FacesStatus`, `BodiesStatus`, `HeldCheckState`,
  `ShadowPlane` y `Calibration`. El crecimiento dentro de v5 está atestiguado
  por el propio v5: `Calibrate` y `Keep` entraron en `LookKind`, y
  `Dispatched` y `Kept` en `LookResult`.
- **cerrado** — el vocabulario del dominio: lo que el cable dice del mundo
  (estados de las máquinas y de las creencias, zonas, lugares, articulaciones,
  los pasos de las cascadas, la trama percibida de las tablas, los tipos de
  hecho de la cinta). Una variante nueva es un cambio incompatible del
  contrato: versión mayor con ADR (regla 13), y por eso queda exhaustivo — el
  compilador señala cada decisión del core y de cada consumidor cuando ese
  cambio llegue. Son `Zone`, `Mode`, `Phase`, `BedState`, `PostureState`,
  `LyingState`, `PartsReading`, `BedPostureState`, `BedPostureSeen`,
  `HeadPlaceState`, `TorsoShadow`, `LegsSide`, `PartsStep`, `TurnRule`,
  `BedSite`, `Joint`, `SchemaId`, `FactBody`, `DerivedBody`, `SensorBody`,
  `MapBody` y `Event`.

**Guardián.** `crates/scene-contract/src/surface.rs` lista cada enum publicado
con su clase y falla si uno nuevo no está clasificado o trae la forma de la
otra clase (`cargo test -p scene-contract`).

**JSON**: sin cambios de valores ni de nombres; `#[non_exhaustive]` no toca el
schema.

**API Rust**: churn de la clasificación, solo lo que ella exige —
`HeldCheckState` gana `#[non_exhaustive]` (abierto); `Phase`, `BedState`,
`LyingState`, `Joint`, `BedPostureSeen` y `SchemaId` lo pierden (cerrados:
son vocabulario del dominio, y hoy lo llevaban sin principio).

**Por qué**: con la mezcla de hoy, un consumidor no podía saber qué enums iban
a crecer y cuáles no; ahora la forma de cada enum lo dice, y dice lo que la
regla 13 ya decía: lo cerrado solo cambia versionado.

**Consumidor con el formato viejo**: sin efecto.

**Consumidor Rust viejo**: sus brazos `_` sobre los enums abiertos siguen
haciendo falta; los que cubrían `Phase`, `BedState`, `LyingState`, `Joint`,
`BedPostureSeen` o `SchemaId` quedan sin efecto (`unreachable_patterns`) y se
quitan — p. ej. el brazo de `calipso-golden/distill.rs` que anticipaba «a bed
state the language does not know yet».

### 2. `UnitError` con la forma abierta

- **JSON**: no es wire; sin efecto.
- **API Rust**: `scene_core::units::UnitError` gana `#[non_exhaustive]` (es un
  error Rust, y sus comprobaciones pueden nombrar nuevas formas de rechazo).
  Su único `match` es el `Display`, dentro del propio `scene-core`: el
  atributo no exige brazos dentro del crate que define el enum, y el match
  queda exhaustivo (más fuerte, no más débil); `detector/src/config/validate.rs`
  lo usa solo por `Display`, sin cambios.
- **Por qué**: un error que puede crecer no debe romper a quien lo formatea.
- **Consumidor Rust viejo**: solo se le ensancha la API; nada que hacer.

### 3. `Display` delegado en `as_str`

- **JSON**: sin cambios.
- **API Rust**: `BedPostureState` y `BedPostureSeen` (`scene-contract`) ganan
  `Display`, delegado en su `as_str`, como `LookKind` ya tenía; en la
  cáscara, `publish::Kind` y `publish::Version` (`detector`) ganan `Display`
  delegado en `as_str`/`token`. Se añade `Display`, no se quita.
- **Por qué**: el nombre de una celda, el del sujeto y el mostrado eran tres
  maneras de escribir lo mismo; ahora escriben una.
- **Consumidor Rust viejo**: solo se le ensancha la API.

### 4. `kind()` unificado en `scene-look`

- **JSON**: sin cambios.
- **API Rust**: sin cambios de paths; `Probe::kind()` y `SenseProbe::kind()`
  leen una sola función privada (`recorded`), donde vive el único mapeo
  propósito→`LookKind`. El ADR quien-responde-cada-probe se conserva: los
  matches nombran todas las variantes, sin wildcard.
- **Por qué**: el mismo mapeo estaba escrito dos veces; un sentido nuevo se
  agrega en un solo lugar.
- **Consumidor Rust viejo**: sin efecto.

### 5. El protocolo del verificador, escrito y probado

- **JSON**: sin cambios; `calipso.verify.v1` ya se hablaba así.
- **API Rust**: `detector/src/verifier/verify_wire.rs` reúne las formas del
  cable — `request_headers()`, `frames_payload()` y `read_answer()` — con
  pruebas de ida y vuelta (cabeceras `Calipso-*` con y sin ráfaga, payload de
  N JPEGs con sus longitudes, y la respuesta leída por su primera palabra
  alfabética: `yes`/`si`/`sí` afirmativo, el resto `unsure`, salvo `no`). Los
  nombres de las pruebas movidas se conservan.
- **Por qué**: el protocolo estaba en el código y en la memoria; ahora está
  escrito en `contract/verify-v1.md` y probado en su módulo.
- **Consumidor con el formato viejo**: sin efecto (el verificador habla igual).
- **Consumidor Rust viejo**: sin efecto.

### 6. `scene-contract` particionado en submódulos por dominio

- **JSON**: sin cambios.
- **API Rust**: los tipos viven en submódulos por dominio (`time`, `run`,
  `zone`, `reception`, `look`, `evidence`, `body`, `posture`, `parts`, `bed`,
  `shadow`, `map`, `room`, `cell`) y la raíz los re-exporta uno a uno: los
  paths públicos (`scene_contract::X`, `scene_contract::v5::X`) son los
  mismos. Los doc-strings se movieron con sus tipos, literalmente (los
  snapshots del schema ni se regeneraron). El `public_api.txt` se actualizó a
  propósito: los mismos 87 nombres, en el mismo orden, ahora escritos como
  re-exports.
- **Por qué**: un `lib.rs` de 1500 líneas no se navega; cada concepto del
  vocabulario tiene su dominio, y la raíz es la superficie.
- **Consumidor Rust viejo**: sin efecto; ni se enteran de la partición.

### 7. Ningún texto cuenta la versión retirada

- **JSON**: sin cambios.
- **API Rust**: sin cambios; solo docs y la descripción del paquete: la
  descripción de `scene-contract` (que nombraba la versión retirada del
  wire), la cabecera de `scene-core/src/project/mod.rs` («today», «sin
  depender de») y el bloque de vocabulario de `scene-core/src/lib.rs`, que
  decía que el dominio era el dueño del vocabulario y que cada versión del
  contrato lo re-exportaba. Hoy los dueños son los crates del lenguaje
  (`scene-contract`, `scene-look`, `scene-geom`; ADR 2026-11-02,
  crates-del-lenguaje) y el core re-exporta lo que decide, para no mover sus
  paths. Todo dice v5 y coincide con el código.
- **Por qué**: un texto que cuenta otra versión que la que corre engaña al que
  entra.
- **Consumidor Rust viejo**: sin efecto.
