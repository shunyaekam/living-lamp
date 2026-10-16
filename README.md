# Living lamp

A lamp that behaves like a small organism instead of a light switch: it breathes, drifts through the day, stirs now and
then, reacts to events, and can follow your screen or your music. It runs on top of
[lampbus](https://github.com/shunyaekam/lampbus), which does the Bluetooth, the protocol and the safety; these apps only
send it colour over UDP, so nothing here can break the lamp control layer.

| App | What it does |
|---|---|
| `lampbus-life` | The creature: moods, breathing, drift, pokes, and a hidden Game of Life as its nervous system. See [docs/LIFE.md](docs/LIFE.md). |
| `lampbus-screen` | Ambient lighting from a display (macOS, choose with `--display N`). |
| `lampbus-music` | Music-reactive lighting from an audio input. |

## Use it

1. Run [lampbus](https://github.com/shunyaekam/lampbus) and enable its UDP input: `"udp_stream": "127.0.0.1:8789"` in
   its config.
2. `cargo build --release`, then `target/release/lampbus-life` (or `lampbus-screen`, `lampbus-music`).
3. To keep the creature running, `./deploy/install.sh` installs it as a launchd (macOS) or systemd (Linux) user service.

The wire format is three to five bytes per frame: `r g b`, `r g b brightness%`, or `r g b hi lo` (brightness in
hundredths of a percent). A stream that goes quiet for two seconds is dropped by lampbus, so senders repeat their last
frame.

## Status

Early. The apps are verified on one lamp, an iStrip/SSL Bluetooth strip. Colour and brightness limits are safe defaults
(the creature's ceiling is 40%), and a person's command always wins over a stream.

Licensed under MIT or Apache-2.0, at your option.
