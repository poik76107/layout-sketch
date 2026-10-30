# Installation

## Download a prebuilt binary

Each [GitHub release](https://github.com/poik76107/layout-sketch/releases/latest) has archives for Linux x86_64 and arm64 (glibc), macOS arm64 and Windows x86_64, plus `SHA256SUMS`. An archive contains the `sketch` executable, the complete `skills/layout-sketch/` directory, `README.md` and `LICENSE`. Unpack it, verify the checksum, and put `sketch` on your `PATH`:

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf sketch-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
install -m 755 sketch-v0.1.0-x86_64-unknown-linux-gnu/sketch ~/.local/bin/sketch
sketch --version
```

Then [install the skill](#install-the-skill) from the unpacked `skills/layout-sketch/` directory.

## Build and install the binary

Use a current stable Rust toolchain supporting edition 2024. The first Cargo build may download dependencies; `sketch` runs offline. From the checkout root:

```sh
cargo build --release
SKETCH_BIN_DIR="${HOME}/.local/bin"
mkdir -p "$SKETCH_BIN_DIR"
cp ./target/release/sketch "$SKETCH_BIN_DIR/sketch"
export PATH="$SKETCH_BIN_DIR:$PATH"
sketch --version
sketch --help
```

Ensure that directory is in the agent's PATH, and check both commands in the agent's execution environment. Alternatively tell the agent the executable's path. This copies the binary only; it does not install the skill.

## Install the skill

Choose one destination for the client. These destinations are in your own agent project or home, respectively:

| Client | Project | User |
| --- | --- | --- |
| Codex | `.agents/skills/layout-sketch/` | `~/.agents/skills/layout-sketch/` |
| Claude Code | `.claude/skills/layout-sketch/` | `~/.claude/skills/layout-sketch/` |
| OpenCode | `.opencode/skills/layout-sketch/` | `~/.config/opencode/skills/layout-sketch/` |

From the checkout root, set `SKETCH_SKILLS_DIR` to the parent of the chosen destination, then copy the whole directory. For example, set it to an absolute project `.agents/skills` path for Codex. This refuses an existing destination; preserve and deliberately upgrade an existing installation instead of creating competing copies.

```sh
SKETCH_SKILLS_DIR="/path/to/agent-project/.agents/skills"
mkdir -p "$SKETCH_SKILLS_DIR"
test ! -e "$SKETCH_SKILLS_DIR/layout-sketch" && cp -R skills/layout-sketch "$SKETCH_SKILLS_DIR/"
```

The destination must contain `SKILL.md`, `references/`, `assets/`, and `agents/`. Check skill discovery and restart the client if needed. In client chat, use `$layout-sketch` in Codex, `/layout-sketch` in Claude Code, or ask OpenCode to apply the `layout-sketch` skill. These are chat invocations, not shell commands.

The agent needs local execution, JSON/PNG file access and an image-view tool connected to a multimodal model. Installing the skill does not grant vision. For a smoke test, copy the shipped `01-basic.json` to a working directory, ask the agent to render it and inspect the preview for two objects and their connector, then change one object's position or size and inspect the new preview. Enlarge or inspect the full-size image if details are unclear.

For image generation, inspect the preview from a successful matching scene and render, enlarging it or inspecting the full-size image when needed. Pass the reviewed clean full-size PNG through the installed imagegen interface; keep style instructions separate from the layout reference. If imagegen called this workflow, return the handoff to it so it can resume. See the [handoff reference](../skills/layout-sketch/references/handoff.md) for details. Copying the skill does not configure imagegen.
