# jigsaw

A small Rust app that generates an M×N grid of interlocking jigsaw pieces, lets you scramble and snap them on a free canvas, and exports JSON for a separate solver.

It includes:

- A native desktop UI
- The same [egui](https://www.egui.rs/) UI compiled to WebAssembly
- A docs site that embeds the web app and explains the piece model

## Documentation

Play in the browser, plus how generation, snapping, and export work:

**https://vesuvisian.com/jigsaw/**

Source for the site lives under [`docs/`](docs/). Start with [`docs/index.md`](docs/index.md) for the playground, then [`docs/how-it-works.md`](docs/how-it-works.md) for the piece model.

### Local documentation

The docs site embeds the Trunk build from `docs/app/` (gitignored; rebuilt here and in CI).

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk

python3 -m venv .venv
source .venv/bin/activate
pip install zensical

env -u NO_COLOR trunk build --release
zensical serve
```

Then open the provided URL (usually `http://127.0.0.1:8000/jigsaw`).

If Trunk errors on `--no-color` (some environments set `NO_COLOR=1`), keep `env -u NO_COLOR` on the `trunk` invocation.

Desktop: `cargo run`. Web app only: `trunk serve` (http://127.0.0.1:8080).
