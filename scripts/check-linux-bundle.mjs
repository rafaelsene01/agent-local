#!/usr/bin/env node
// Answers Open Question #1 of .specs/features/self-contained-runtime/design.md
// with evidence instead of a guess: does a Tauri `.deb` keep the execute bit on
// the vendored `llama-server`?
//
// No documentation was found either way, so the app does not depend on the
// answer — `runtime::bundled::ensure_executable` sets the bit itself, and falls
// back to a writable copy when it cannot. This script exists to record which of
// the two paths a real package actually takes.
//
// Usage:
//   node scripts/check-linux-bundle.mjs <package.deb>

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";

/** One `dpkg -c` line, e.g.
 *  `-rwxr-xr-x root/root  1234 2026-07-27 10:00 ./usr/lib/LocalMind/resources/llama/vulkan/llama-server` */
export function parseDpkgLine(line) {
  const match = line.trim().match(/^(\S{10})\s+\S+\s+(\d+)\s+\S+\s+\S+\s+(\.\/\S+)/);
  if (!match) return null;
  const [, mode, size, path] = match;
  return { mode, size: Number(size), path, executable: mode.includes("x") };
}

export function findEntries(listing, fileName) {
  return listing
    .split("\n")
    .map(parseDpkgLine)
    .filter((entry) => entry && entry.path.endsWith(`/${fileName}`));
}

function main(argv) {
  const pkg = argv[0];
  if (!pkg) throw new Error("usage: check-linux-bundle.mjs <package.deb>");
  if (!existsSync(pkg)) throw new Error(`package not found: ${pkg}`);

  const listing = execFileSync("dpkg", ["-c", pkg], { encoding: "utf8" });
  const servers = findEntries(listing, "llama-server");

  // Absence is a packaging failure: the app cannot answer anything without it.
  // A missing execute bit is not — the app repairs that at run time — so it is
  // reported loudly and does not fail the build.
  if (servers.length === 0) {
    throw new Error(
      "llama-server is not inside the .deb — bundle.resources did not pick up src-tauri/resources/",
    );
  }

  for (const entry of servers) {
    const verdict = entry.executable ? "executable" : "NOT executable (ensure_executable will repair)";
    console.log(`check-linux-bundle: ${entry.path} ${entry.mode} ${entry.size} B — ${verdict}`);
  }

  for (const name of ["libonnxruntime.so", "libpdfium.so"]) {
    const found = findEntries(listing, name);
    console.log(
      found.length > 0
        ? `check-linux-bundle: ${name} present (${found[0].mode})`
        : `check-linux-bundle: WARNING — ${name} is not in the package`,
    );
  }
}

if (process.argv[1] && process.argv[1].endsWith("check-linux-bundle.mjs")) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`check-linux-bundle: ${error.message}\n`);
    process.exit(1);
  }
}
