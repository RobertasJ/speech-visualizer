# speech-visualizer

A Freya (0.5 rc) desktop app that transcribes the microphone with whisper.cpp
(`whisper-rs`) and, in live mode, turns spoken commands into elements shown in a
separate display window.

## Working here

- The user makes the structural choices (files, modules, splitting things up). Explain
  and map the code, but only restructure when asked for a specific change.
- Version control is jj, colocated with git. Use `jj` commands, not `git`.
- The dev shell comes from `flake.nix` via direnv (bindgen, CUDA, Vulkan env vars).
- Commands are in the `justfile`: `just run`, `just run-cuda`, `just run-vulkan`,
  `just clippy`, `just fmt`. Tests: `cargo test`.
- Models (`ggml-*.bin`) live in `models/`, which is gitignored. The first CLI arg
  overrides the directory. Files with "silero" in the name are VAD models.

## Layout

- `stt.rs`: `run()` loads the models, captures audio with cpal, and does VAD and
  whisper passes until stopped, blocking. It emits `Event::Started`, then `Live` /
  `Final` / `Error` (stream errors, recording goes on), and returns an `Error` on failure.
  - `stt/mic.rs`: `Mic::start()` records mono audio from the default input device with
    cpal; `recv(timeout)` waits for it and returns everything queued. It stops when
    dropped.
  - `stt/ui/`: the Freya side of it. `use_stt(on_event)` runs `run()` on a thread and
    calls `on_event` on the UI thread for each event, ending with `Event::Failed` if it
    errors. Also `Status` (for screens to show) and `Diagnostics`.
- `scene.rs`: the element tree.
  - `scene/command.rs` / `scene/names.rs`: parse and apply text commands to a scene;
    names map words to element ids.
  - `scene/ui/`: `use_scene()` creates a scene owned by the calling component, and the
    display window (`display.rs`) lives as long as it does. `window.rs` has
    `spawn_window` / `WindowHandle`, a window that closes when its handle is dropped.
- `live.rs`: `use_live(scene, names)` runs `use_stt` and turns final transcription text
  into commands for the scene. Returns a `UseLive` handle (`status()` for screens).
- `options.rs`: the choices made on the selection screen.
- `ui/`: one component per screen (`Selection`, `Session`, `Console`, `LiveSession`)
  plus `NavBack`.
- Each module's UI code goes in its own `ui/` subfolder. The modules re-export what
  callers need (`scene::{Command, Names, use_scene}`, `stt::use_stt`).
- Unit tests are inline at the bottom of their module, in `#[cfg(test)] mod tests`.
  Modules use `foo.rs` + `foo/`, never `mod.rs`.

## Patterns

- `Options` is a global context (`State::create_global` + `LaunchConfig::with_global`),
  read with `GlobalContexts::get().get_context::<State<Options>>()`. It is shared by
  every window and kept for the whole run.
- The current `Screen` is provided as context in `SpeechApp` and read with
  `use_consume::<State<Screen>>()`.
- Resources tied to a screen are hooks owned by the screen's component (`use_scene`,
  `use_stt`), which clean up when it unmounts. Don't thread them through props
  or globals.

## Stopping speech to text

- If the process exits while whisper is still using the GPU, ggml aborts. So the stt
  thread must be stopped and joined before exit.
- `use_stt` sets the stop flag and joins the thread synchronously in `use_drop`. Freya runs `use_drop` when a
  window closes, before the event loop exits, so this also covers quitting.
- Stopping takes less than a frame, so the synchronous join is fine. Don't move it to a
  background thread.
