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

## F1c — pendiente

Los cambios de forma de F1c se anotan aquí, con la misma estructura, al
cerrarse ese frente.
