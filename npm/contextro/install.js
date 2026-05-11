#!/usr/bin/env node
"use strict";

const https = require("https");
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");
const { execSync } = require("child_process");

const REPO = "jassskalkat/contextro";
const pkg = require("./package.json");
const VERSION = `v${pkg.version}`;

const PLATFORM_MAP = {
  darwin: { arm64: "aarch64-apple-darwin", x64: "x86_64-apple-darwin" },
  linux: { arm64: "aarch64-unknown-linux-gnu", x64: "x86_64-unknown-linux-gnu" },
  win32: { x64: "x86_64-pc-windows-msvc" },
};

function getTarget() {
  const targets = PLATFORM_MAP[process.platform];
  if (!targets) throw new Error(`Unsupported platform: ${process.platform}`);
  const target = targets[process.arch];
  if (!target) throw new Error(`Unsupported arch: ${process.arch} on ${process.platform}`);
  return target;
}

function fetch(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { "User-Agent": "contextro-npm-installer" } }, (res) => {
      if (res.statusCode === 301 || res.statusCode === 302) {
        return fetch(res.headers.location).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    }).on("error", reject);
  });
}

async function main() {
  const target = getTarget();
  const isWindows = process.platform === "win32";
  const archive = isWindows
    ? `contextro-${target}.zip`
    : `contextro-${target}.tar.gz`;
  const url = `https://github.com/${REPO}/releases/download/${VERSION}/${archive}`;
  const binDir = path.join(__dirname, "bin");
  const binPath = path.join(binDir, isWindows ? "contextro.exe" : "contextro");

  fs.mkdirSync(binDir, { recursive: true });

  console.log(`Downloading contextro ${VERSION} (${target})...`);
  const data = await fetch(url);

  if (isWindows) {
    // Write zip and extract with PowerShell
    const zipPath = path.join(binDir, archive);
    fs.writeFileSync(zipPath, data);
    execSync(
      `powershell -Command "Expand-Archive -Force '${zipPath}' '${binDir}'"`,
      { stdio: "inherit" }
    );
    fs.unlinkSync(zipPath);
  } else {
    // Extract tar.gz in-process
    await new Promise((resolve, reject) => {
      const gunzip = zlib.createGunzip();
      const chunks = [];
      gunzip.on("data", (c) => chunks.push(c));
      gunzip.on("end", () => {
        // Minimal tar parser: find the binary entry and write it
        const buf = Buffer.concat(chunks);
        let offset = 0;
        while (offset + 512 <= buf.length) {
          const name = buf.slice(offset, offset + 100).toString("utf8").replace(/\0/g, "");
          const sizeOctal = buf.slice(offset + 124, offset + 136).toString("utf8").replace(/\0/g, "").trim();
          const size = parseInt(sizeOctal, 8) || 0;
          offset += 512;
          if (name === "contextro" || name === "./contextro") {
            fs.writeFileSync(binPath, buf.slice(offset, offset + size), { mode: 0o755 });
            break;
          }
          offset += Math.ceil(size / 512) * 512;
        }
        resolve();
      });
      gunzip.on("error", reject);
      gunzip.end(data);
    });
  }

  if (!fs.existsSync(binPath)) {
    throw new Error("Binary not found after extraction");
  }

  console.log(`✓ contextro installed`);
}

main().catch((err) => {
  console.error(`contextro install failed: ${err.message}`);
  console.error("You can install manually: https://github.com/jassskalkat/contextro#install");
  // Don't fail npm install — the package still works if the binary is installed separately
  process.exit(0);
});
