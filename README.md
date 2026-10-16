# Living lamp

A lamp that behaves like a small organism instead of a light switch. It breathes, wanders slowly inside gentle limits, and
**every input is a feed into it**: your commands (Siri, the CLI, an app), the screen, an agent's state. The organism is the
only thing that drives the lamp, through [lampbus](https://github.com/shunyaekam/lampbus), which does the Bluetooth, the
protocol and the safety. What it is and why is in [docs/MODEL.md](docs/MODEL.md).

```
the world and the person  ─ feeds ─▶  living-lamp (organism)  ─ colour ─▶  lampbus  ─▶  lamp
(commands, screen, agent)             L2 alignment, L1 spontaneous,        UDP stream
                                      L3 imprint, then the L0 limits       input
```

| Part | What it is |
|---|---|
| `apps/organism` (`living-lamp`) | The organism: feeds in over UDP, colour out to lampbus. |
| `crates/lamp-model` | Its inner model and the experiments behind the design, with no lamp, no network and no clock. |
| `apps/screen` (`lampbus-screen`) | Ambient lighting from a display (macOS, `--display N`); point `--to` at the organism's screen port. |
| `apps/music` (`lampbus-music`) | Music-reactive lighting from an audio input. |
| `apps/life` (`lampbus-life`) | The earlier standalone creature (moods, pokes, a Game of Life), which talks to lampbus without owning it. |

## Run it

1. Install [lampbus](https://github.com/shunyaekam/lampbus) and set `"udp_stream": "127.0.0.1:8789"` in its config.
2. `cargo build --release`, then `./deploy/install.sh` (installs `living-lamp` as a launchd or systemd user service).
3. To let it own the lamp, add `"controller": "127.0.0.1:8791"` to lampbus's config and restart lampbus. Every command that
   reaches lampbus then goes to the organism, which answers `ok` or `err` (so an organism that is not running is an error you
   see), and the lamp shows what it streams back.
4. **The way out:** `lampbus direct on` makes lampbus execute commands itself again and bypasses the organism;
   `lampbus direct off` hands the lamp back. `LAMPBUS_DIRECT=1` does the same at start.

Talk to it directly with `living-lamp send`:

```console
$ living-lamp send person look 255 0 0 40     # a person chooses red at 40%
$ living-lamp send person brightness 80
$ living-lamp send person power off           # or on, toggle
$ living-lamp send release                    # let go: it eases back to its own ember in about two minutes
$ living-lamp send agent working              # working, done, needs-you, error, idle
$ living-lamp send status
```

`tools/agent-presence.sh working|done|needs-you|error|idle` wraps the agent messages for use from hooks. The screen app
sends to the organism with `lampbus-screen --to 127.0.0.1:8792 --creature off`.

## What to know

- If the organism stops, commands stop (they fail with a clear message) until it restarts; the service restarts itself, and
  lampbus keeps the last light. `lampbus direct on` is the switch for a person.
- A person's command takes the lamp for about an hour and then hands it back gradually; the screen yields to a person.
- The organism's own light is a slow ember capped at 40%; a person who asks for more gets it.
- It keeps what it has learned across restarts in `~/.local/state/living-lamp/model`.

## Status

Early. Verified on one lamp, an iStrip/SSL Bluetooth strip. The model's results so far are on synthetic feeds (see
`docs/MODEL.md`, section 9); the feeds that matter (weather, music) are not built yet.

Licensed under MIT or Apache-2.0, at your option.
