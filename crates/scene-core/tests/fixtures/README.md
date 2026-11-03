# Fixtures de `scene-core`

## `cascade-dig1.json` (paridad de la cascada, F2.2)

Los 220 cuerpo-segundo de dig1 con lo que decide `classify_s1` del prototipo
(`workshop/43-ocupacion-por-partes/herramientas/partes.py`): la bandera `E5S`,
reglas `E2, E5, E5S, E6, A, B, D`, sin memoria, con los valores propuestos de
`resultados/reglas.md` (franja 0,18 m, score 0,75, share 0,45, E2 0,6–1,4).
La prueba `room::cascade::tests::the_cascade_agrees_with_the_prototype_on_dig1`
exige igualdad con él.

Para regenerarlo (desde la raíz del workspace; `uv` instala numpy y opencv en
un entorno temporal; no escribe en el taller):

```sh
cd calipso-detector/crates/scene-core/tests/fixtures
uv run --no-project --with numpy --with opencv-python-headless python generate-cascade-dig1.py cascade-dig1.json
```

El generador lee `workshop/43-ocupacion-por-partes/perfiles/dig1.toml` y
`cache/cuadro-entero/pose-dig1.jsonl`. Si cambia el prototipo o el caché, el
fixture cambia y la prueba debe seguir igual o explicar la diferencia.
