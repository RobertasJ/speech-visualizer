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

- `stt.rs`: `Transcriber`, audio capture with cpal, VAD, and whisper passes on a
  worker thread. It emits `Event::Live`, `Event::Final` and `Event::Error`.
  - `stt/ui/`: the Freya side of it, `use_transcriber` + `Status` and `Diagnostics`.
- `scene.rs`: the element tree.
  - `scene/command.rs` / `scene/names.rs`: parse and apply text commands to a scene;
    names map words to element ids.
  - `scene/ui/`: `use_scene()` creates a scene owned by the calling component, and the
    display window (`display.rs`) lives as long as it does. `window.rs` has
    `spawn_window` / `WindowHandle`, a window that closes when its handle is dropped.
- `live.rs`: turns final transcription text into commands.
- `options.rs`: the choices made on the selection screen.
- `ui/`: one component per screen (`Selection`, `Session`, `Console`, `LiveSession`)
  plus `NavBack`.
- Each module's UI code goes in its own `ui/` subfolder. The modules re-export what
  callers need (`scene::{Command, Names, use_scene}`, `stt::use_transcriber`).
- Unit tests are inline at the bottom of their module, in `#[cfg(test)] mod tests`.
  Modules use `foo.rs` + `foo/`, never `mod.rs`.

## Patterns

- `Options` is a global context (`State::create_global` + `LaunchConfig::with_global`),
  read with `GlobalContexts::get().get_context::<State<Options>>()`. It is shared by
  every window and kept for the whole run.
- The current `Screen` is provided as context in `SpeechApp` and read with
  `use_consume::<State<Screen>>()`.
- Resources tied to a screen are hooks owned by the screen's component (`use_scene`,
  `use_transcriber`), which clean up when it unmounts. Don't thread them through props
  or globals.

## Transcriber shutdown

- If the process exits while whisper is still using the GPU, ggml aborts. So the
  transcriber must be dropped (its thread joined) before exit.
- `use_transcriber` drops it synchronously in `use_drop`. Freya runs `use_drop` when a
  window closes, before the event loop exits, so this also covers quitting.
- Stopping takes less than a frame, so the synchronous drop is fine. Don't move it to a
  background thread.
