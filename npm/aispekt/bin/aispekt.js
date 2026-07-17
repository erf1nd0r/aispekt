#!/usr/bin/env node
// Thin launcher: resolves the platform-specific native binary installed via
// optionalDependencies (the esbuild/Biome distribution pattern) and forwards
// argv, stdio, and the exit code untouched.
"use strict";
const { execFileSync } = require("node:child_process");

const PLATFORMS = {
  "darwin arm64": "aispekt-darwin-arm64",
  "darwin x64": "aispekt-darwin-x64",
  "linux x64": "aispekt-linux-x64",
  "linux arm64": "aispekt-linux-arm64",
  "win32 x64": "aispekt-win32-x64",
};

const key = `${process.platform} ${process.arch}`;
const pkg = PLATFORMS[key];
if (!pkg) {
  console.error(
    `aispekt: no prebuilt binary for ${key}. Build from source: cargo install --locked --git https://github.com/erf1nd0r/aispekt aispekt`,
  );
  process.exit(2);
}

let bin;
try {
  bin = require.resolve(`${pkg}/bin/aispekt${process.platform === "win32" ? ".exe" : ""}`);
} catch {
  console.error(
    `aispekt: platform package "${pkg}" is missing. Reinstall with optional dependencies enabled (npm install aispekt), or build from source with cargo.`,
  );
  process.exit(2);
}

try {
  execFileSync(bin, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  process.exit(typeof err.status === "number" ? err.status : 2);
}
