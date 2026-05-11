# Contextro v0.1.0 Distribution Plan

**Status:** `jassskalkat/lagos` merged ✅ — ready to ship  
**Date:** 2026-05-10  
**Version:** 0.1.0 (first stable Rust release; PyPI was 0.0.7)

---

## Context

Contextro was previously distributed as a Python package on PyPI (`pip install contextro`, last version 0.0.7). The entire runtime has been rewritten in Rust. The binary is now a single statically-linked executable with no runtime dependencies. Distribution strategy must change accordingly.

**Binary targets to ship:**

| Target triple | Platform |
|---|---|
| `aarch64-apple-darwin` | macOS Apple Silicon (M1/M2/M3) |
| `x86_64-apple-darwin` | macOS Intel |
| `x86_64-unknown-linux-gnu` | Linux x86_64 |
| `aarch64-unknown-linux-gnu` | Linux ARM64 (servers, Raspberry Pi) |
| `x86_64-pc-windows-msvc` | Windows x86_64 |

---

## Distribution Channels

### 1. GitHub Releases + install script (primary)

**What:** Compiled binaries attached to a GitHub Release, installable via a one-liner.  
**Who:** Everyone. No runtime required.  
**Status:** README already advertises `curl -fsSL https://install.contextro.dev | sh`

**Files needed:**
- `.github/workflows/release.yml` — cross-compile on tag push, create GitHub Release, upload binaries
- `install.sh` — detects OS/arch, downloads the right binary, places it in `~/.local/bin` or `/usr/local/bin`

**Trigger:** Push a git tag `v0.1.0` → CI builds all 5 targets → GitHub Release created automatically.

**User experience:**
```bash
curl -fsSL https://install.contextro.dev | sh
# or
curl -fsSL https://raw.githubusercontent.com/jassskalkat/contextro/main/install.sh | sh
```

---

### 2. crates.io (Rust ecosystem)

**What:** `cargo install contextro` — standard Rust package registry.  
**Who:** Rust developers, CI environments with Rust toolchain.  
**Status:** Not yet published. Requires `cargo publish` with a crates.io API token.

**Files needed:**
- `crates/contextro-server/Cargo.toml` — verify `description`, `license`, `repository`, `homepage` fields are set
- `CRATES_IO_TOKEN` secret in GitHub repo settings
- Add `cargo publish` step to release workflow

**User experience:**
```bash
cargo install contextro
```

**Notes:**
- Only the `contextro-server` crate needs to be published (the binary crate). Internal library crates (`contextro-core`, `contextro-engines`, etc.) can stay private for now.
- crates.io requires all path dependencies to also be published or replaced with version deps. Will need to publish the workspace crates in dependency order.

---

### 3. npm wrapper (widest reach)

**What:** An npm package that downloads the correct platform binary on `npm install`.  
**Who:** JavaScript/TypeScript developers — the primary Contextro audience (Claude Code, Cursor, Windsurf users).  
**Status:** Not yet created.

**How it works** (same pattern as `esbuild`, `@biomejs/biome`, `turbo`):
- `npm/contextro/` — thin JS wrapper package
- `postinstall` script detects `process.platform` + `process.arch`, downloads the matching binary from the GitHub Release, makes it executable
- Optional: platform-specific packages (`@contextro/darwin-arm64`, etc.) as optional dependencies — avoids downloading at install time

**Files needed:**
- `npm/contextro/package.json`
- `npm/contextro/install.js` — postinstall script
- `npm/contextro/bin/contextro` — thin JS shim that execs the binary
- Add `npm publish` step to release workflow (requires `NPM_TOKEN` secret)

**User experience:**
```bash
npm install -g contextro
# or in MCP config — no global install needed:
npx contextro@latest
```

**MCP config with npx (zero install):**
```json
{
  "mcpServers": {
    "contextro": {
      "command": "npx",
      "args": ["-y", "contextro@latest"]
    }
  }
}
```

---

### 4. Homebrew tap (Mac/Linux power users)

**What:** `brew install jassskalkat/tap/contextro`  
**Who:** Mac/Linux developers who prefer Homebrew.  
**Status:** Not yet created. Lower priority.

**Files needed:**
- New repo: `github.com/jassskalkat/homebrew-tap`
- `Formula/contextro.rb` — Homebrew formula pointing to GitHub Release tarballs

**Can be deferred** until there's user demand. The install script covers the same audience.

---

## Implementation Order

```
Step 1: GitHub Actions release workflow        ← unblocks everything
Step 2: install.sh                             ← fulfills README promise
Step 3: npm wrapper                            ← widest reach, do early
Step 4: crates.io publish                     ← easy, do alongside Step 3
Step 5: Homebrew tap                           ← defer, low priority
```

---

## Step 1: GitHub Actions Release Workflow

**File:** `.github/workflows/release.yml`

**Trigger:** Push tag matching `v*` (e.g. `git tag v0.1.0 && git push --tags`)

**Jobs:**
1. `build` — matrix over 5 targets using `cross` for Linux/Windows cross-compilation
2. `release` — creates GitHub Release, uploads binaries, generates checksums (`SHA256SUMS`)
3. `publish-crates` — runs `cargo publish` for each workspace crate in dependency order
4. `publish-npm` — runs `npm publish` from `npm/contextro/`

**Secrets required:**
- `CRATES_IO_TOKEN` — from https://crates.io/settings/tokens
- `NPM_TOKEN` — from https://www.npmjs.com/settings/tokens

---

## Step 2: install.sh

**Location:** `install.sh` in repo root (also served at `install.contextro.dev`)

**Logic:**
1. Detect OS (`uname -s`) and arch (`uname -m`)
2. Map to target triple
3. Download `https://github.com/jassskalkat/contextro/releases/download/v{VERSION}/contextro-{TARGET}.tar.gz`
4. Verify SHA256 checksum
5. Extract binary to `~/.local/bin/contextro` (Linux) or `/usr/local/bin/contextro` (macOS)
6. Print success + MCP config snippet

---

## Step 3: npm wrapper

**Package name:** `contextro` on npm (check availability — may need `@contextro/mcp` if taken)

**Structure:**
```
npm/contextro/
├── package.json          # name, version, bin, scripts.postinstall
├── install.js            # postinstall: detect platform, download binary
├── run.js                # bin shim: spawn the binary with process.argv
└── README.md
```

**package.json `bin` field:**
```json
{
  "bin": { "contextro": "./run.js" }
}
```

---

## Step 4: crates.io

**Publish order** (dependencies first):
1. `contextro-core`
2. `contextro-config`
3. `contextro-parsing`
4. `contextro-indexing`
5. `contextro-engines`
6. `contextro-memory`
7. `contextro-git`
8. `contextro-tools`
9. `contextro-server` (the binary)

**Required fields in each `Cargo.toml`:**
```toml
description = "..."
license = "MIT"  # or proprietary — check LICENSE file
repository = "https://github.com/jassskalkat/contextro"
homepage = "https://contextro.dev"
```

---

## Versioning Going Forward

| Channel | Current | Next patch | Next minor |
|---|---|---|---|
| GitHub Releases | — | `v0.1.0` | `v0.2.0` |
| crates.io | — | `0.1.0` | `0.2.0` |
| npm | — | `0.1.0` | `0.2.0` |
| PyPI | `0.0.7` | deprecated | — |

PyPI package should be marked as deprecated with a message pointing to the new install method.

---

## Files to Create

```
.github/
  workflows/
    release.yml          ← CI: build, release, publish
    ci.yml               ← CI: cargo test on every PR (already needed)
install.sh               ← curl-pipe install script
npm/
  contextro/
    package.json
    install.js
    run.js
    README.md
```

---

## Open Questions

1. **npm package name** — is `contextro` available on npm? Check before building the wrapper.
2. **License** — crates.io requires a valid SPDX license identifier. Current `LICENSE` file says "Proprietary". Options: keep proprietary (can still publish to crates.io with `license = "LicenseRef-Proprietary"`), or switch to MIT/Apache-2.0.
3. **`install.contextro.dev` domain** — does this exist? If not, use the raw GitHub URL in the README until it's set up.
4. **Windows support** — the binary compiles for Windows but hasn't been tested. May need a separate `install.ps1` PowerShell script.
