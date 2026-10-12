Ya está casi todo corriendo. Verificado recién:

* `go2rtc` vivo en `/home/visiona/opt/media` (`go2rtc.yaml` sirve `sample1: ffmpeg:videos/sample1_gop6.mp4` en `:8554`) → coincide con `config/sample1.toml: `rtsp://127.0.0.1:8554/sample1``.
* `nats-server` vivo en `:4222` → coincide con `[nats] url` del toml.
* `--check` de `sample1.toml` en verde (perfil con detect + reinforce + faces + pose + depth + calibrate).

## Levantarlo

Desde `calipso-detector/`:

```sh
# 1. (opcional) validar el perfil, sin abrir RTSP/modelos/NATS
cargo run -p detector --bin calipso-detector -- --config config/sample1.toml --check

# 2. correr contra sample1
CALIPSO_MODELS=/home/visiona/mana/calipso/calipso-dist/models \
  cargo run -p detector --bin calipso-detector -- --config config/sample1.toml
```

`CALIPSO_MODELS` apunta a `calipso-dist/models`, que ya tiene los 5 modelos que pide el toml (`yolo26s.onnx`, `yolo26l.onnx`, `face/yolov12l-face.onnx`, `yolo26s-pose.onnx`, `yolo26s-depth.onnx`). Sin esa variable los busca relativos al cwd y falla.

## Qué publica

Prefijo por defecto `vision.<site>.<id>` → con este toml (`site="local"`, `id="sample1"`):

* `vision.local.sample1.scene.v4`
* `vision.local.sample1.events.v4`

Una escena por muestra (~1/s, `sample_interval_ms = 1000`). Cierre con `Ctrl-C` / `SIGTERM`: termina la vuelta en curso y da hasta 3 s al publisher para vaciar (`crates/detector/README.md:48-56`).

Nota: no tienes el CLI `nats` en PATH (`which nats` → no encontrado), así que para ver lo publicado usa `calipso-rerun-bridge` (ya lee v4) o `calipso-shark` desde NATS, no `nats sub`.


--- 


Correcto, es `../calipso-rerun-bridge` (lee v4, probado el 2026-10-11 contra sample1 según su README).

Para ver sample1 en vivo, 3 terminales:

```sh
# T1 — detector (ya lo tienes)
CALIPSO_MODELS=/home/visiona/mana/calipso/calipso-dist/models \
  cargo run -p detector --bin calipso-detector -- --config config/sample1.toml
```

```sh
# T2 — visor. OJO: el `rerun` del PATH es 0.33.1 y el README pide 0.37.x
# (SDK fijado en Cargo.lock). Usa el binario 0.37.0 que ya está en calipso/:
../rerun-cli-0.37.0-x86_64-unknown-linux-gnu --port 9876
```

```sh
# T3 — bridge, desde ../calipso-rerun-bridge
NATS_URL=nats://127.0.0.1:4222 \
NATS_SUBJECT='vision.local.sample1.*.v4' \
RERUN_ADDRESS=127.0.0.1:9876 \
cargo run --locked
```

Alternativa sin visor en vivo (graba a disco y lo abres después):

```sh
# en ../calipso-rerun-bridge
NATS_URL=nats://127.0.0.1:4222 \
NATS_SUBJECT='vision.local.sample1.*.v4' \
RERUN_OUTPUT=/tmp/vivo.rrd \
cargo run --locked
../rerun-cli-0.37.0-x86_64-unknown-linux-gnu /tmp/vivo.rrd
```

Nota: una sola instancia del detector por cámara — si ves que el bridge alterna dos `run_id` / abre grabaciones repetidas, mata la instancia vieja (`pgrep -af calipso-detector`).
