# speech-visualizer

A Freya (0.5 rc) desktop app that transcribes the microphone with whisper.cpp
(`whisper-rs`) and, in live mode, has an agent turn what's said into elements shown in a
separate display window, by writing Rhai scripts.

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
- `scene.rs`: the element tree. Elements can have a description. It serializes for the
  agent as a nested tree, with `next_id`.
  - `scene/script.rs`: `Script` runs Rhai on a scene through a `Store` (for
    `State<Scene>` in `scene/ui.rs`; for `Rc<RefCell<Scene>>` in tests). Variables last
    between runs, a failed run is undone (the scene, and a deep copy of the variables),
    and `fn`s only last for their run. Engine limits keep scripts from hanging the UI
    thread.
    - `script/api.rs`: the functions and the `Element` handle scripts use. Every use of
      a removed element throws `no element <id>`. `print` and `debug` are registered for
      elements so printing throws too: rhai's own show a failing `to_string` as just
      the type name, which still happens in string interpolation, `+` and arrays.
    - `script/table.rs`: `FUNCTIONS`, one line each, for the console's help and the
      agent's prompt. Keep it in step with `api.rs`.
    - `script/show.rs`: values as text, for results and `variables()`.
  - `scene/ui/`: `use_scene()` creates a scene owned by the calling component, and the
    display window (`display.rs`) lives as long as it does. `window.rs` has
    `spawn_window` / `WindowHandle`, a window that closes when its handle is dropped.
- `live.rs`: live mode. `use_live(scene)` runs `use_stt` and wires the parts below; the
  `UseLive` it returns hands their state to the screen.
  - `live/transcript.rs`: what's been said, in numbered segments, each with its check.
  - `live/check.rs`: asks TypeSafe AI (`typesafeai.rs`, `TYPESAFE_API_KEY`) whether a
    segment asks for anything new and, if the agent is running, whether it corrects it.
  - `live/schedule.rs`: what a checked segment does: start a run, restart the running
    one (a correction), queue a rerun for when it finishes, or nothing.
  - `live/agent.rs`: one run, on Anthropic through rig (`ANTHROPIC_API_KEY`; the model
    and its thinking effort are consts at the top). The agent calls `run_script` and
    gets each outcome and the scene back at once, until a reply makes no call.
    `agent/input.rs` is its first message and system prompt (`agent/preamble.md`),
    `agent/reply.rs` follows a streamed reply, `agent/tools.rs` is the tool.
  - `live/history.rs`: what each run did; shown on screen, and sent back as text.
  - `live/ui/`: `TranscriptPanel` and `ActivityPanel`, the live screen's debug view.
- `typesafeai.rs`: the TypeSafe AI client, with Noul, Choice and Score questions.
- `options.rs`: the choices made on the selection screen.
- `ui/`: one component per screen (`Selection`, `Session`, `Console`, `LiveSession`)
  plus `NavBack`.
- Each module's UI code goes in its own `ui/` subfolder. The modules re-export what
  callers need (`scene::{FUNCTIONS, Outcome, Script, use_scene}`,
  `live::{use_live, TranscriptPanel, ActivityPanel}`, `stt::use_stt`).
- Unit tests are inline at the bottom of their module, in `#[cfg(test)] mod tests`, so
  each part is tested on its own. Components are tested headlessly with
  `freya-testing` (a dev-dependency): provide their state with
  `provide_root_context`, call `use_init_theme`, and read labels with
  `Label::try_downcast`. The glue in `live.rs` and the screens aren't tested, since
  they need the mic, the models or the APIs.
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
