#!/usr/bin/env node
"use strict";

const path = require("path");
const { spawnSync } = require("child_process");

const isWindows = process.platform === "win32";
const bin = path.join(__dirname, "bin", isWindows ? "contextro.exe" : "contextro");

const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error(
      "contextro binary not found. Try reinstalling: npm install -g contextro"
    );
  } else {
    console.error(result.error.message);
  }
  process.exit(1);
}

process.exit(result.status ?? 0);
