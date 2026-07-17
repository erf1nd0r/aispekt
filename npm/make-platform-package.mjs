#!/usr/bin/env node
// Assembles one platform npm package (aispekt-<os>-<cpu>) around a built
// binary. Used by .github/workflows/release.yml; runnable locally:
//
//   node npm/make-platform-package.mjs darwin-arm64 target/release/aispekt 0.2.0 out/
//
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [slug, binaryPath, version, outDir] = process.argv.slice(2);
if (!slug || !binaryPath || !version || !outDir) {
  console.error("usage: make-platform-package.mjs <os-cpu> <binary> <version> <out-dir>");
  process.exit(2);
}

const [os, cpu] = slug.split("-");
const pkgDir = join(outDir, `aispekt-${slug}`);
const binDir = join(pkgDir, "bin");
mkdirSync(binDir, { recursive: true });

const binName = os === "win32" ? "aispekt.exe" : "aispekt";
copyFileSync(binaryPath, join(binDir, binName));
if (os !== "win32") chmodSync(join(binDir, binName), 0o755);

writeFileSync(
  join(pkgDir, "package.json"),
  JSON.stringify(
    {
      name: `aispekt-${slug}`,
      version,
      description: `aispekt native binary for ${os} ${cpu}`,
      license: "MIT",
      repository: { type: "git", url: "git+https://github.com/erf1nd0r/aispekt.git" },
      os: [os],
      cpu: [cpu],
      files: [`bin/${binName}`],
    },
    null,
    2,
  ) + "\n",
);
console.log(`assembled ${pkgDir}`);
