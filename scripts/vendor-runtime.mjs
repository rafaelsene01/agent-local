#!/usr/bin/env node
// Brings the binary components that ship inside the installer into
// src-tauri/resources/ before a build: the llama.cpp server (Vulkan and CPU),
// the ONNX Runtime and pdfium.
//
// Until M9 these three were downloaded by the app on first use, which meant a
// machine without internet — or behind a proxy that blocks GitHub — could never
// finish setting up. See .specs/features/self-contained-runtime/design.md.
//
// Usage:
//   node scripts/vendor-runtime.mjs [--force]
//
// Without --force this is a no-op when .vendor-stamp.json already matches
// vendor.json, so putting it in `beforeBuildCommand` costs nothing per build.

import { createWriteStream, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { basename, dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = join(ROOT, "scripts", "vendor.json");
export const RESOURCES = join(ROOT, "src-tauri", "resources");
export const STAMP_FILE = ".vendor-stamp.json";

/** The four destinations, relative to `resources/`. The layout mirrors what the
 *  old download code created, so the recursive lookup in `runtime::bundled`
 *  keeps working unchanged. */
export const LAYOUT = {
  "llama/vulkan": { component: "llamaCpp", variant: "vulkan" },
  "llama/cpu": { component: "llamaCpp", variant: "cpu" },
  onnxruntime: { component: "onnxruntime" },
  pdfium: { component: "pdfium" },
};

export function hostPlatform(platform = process.platform) {
  if (platform === "win32" || platform === "linux") return platform;
  throw new Error(
    `unsupported platform: ${platform} — LocalMind ships for Windows and Linux only`,
  );
}

/** Resolves one entry of LAYOUT to a concrete download. Throws (rather than
 *  guessing a name) when the manifest has no asset for this platform. */
export function assetFor(manifest, target, platform) {
  const spec = LAYOUT[target];
  if (!spec) throw new Error(`unknown vendor target: ${target}`);

  const component = manifest[spec.component];
  if (!component) throw new Error(`vendor.json has no "${spec.component}" entry`);

  const forPlatform = component.assets?.[platform];
  const asset = spec.variant ? forPlatform?.[spec.variant] : forPlatform;
  if (!asset?.name) {
    const which = spec.variant ? `${spec.component}.${spec.variant}` : spec.component;
    throw new Error(`vendor.json has no asset named for ${which} on ${platform}`);
  }

  return {
    name: asset.name,
    bytes: asset.bytes ?? null,
    url: `https://github.com/${component.repo}/releases/download/${component.tag}/${asset.name}`,
  };
}

/** Build-time artefacts that are never opened by a running app: debug symbols,
 *  import libraries, and C headers. Measured, not assumed — the ONNX Runtime
 *  Windows package extracts to 426 MB, of which a single `onnxruntime.pdb` is
 *  408 MB. Shipping it would have made the installer bigger than everything
 *  else in the app put together. */
const BUILD_ONLY_EXTENSIONS = [".pdb", ".lib", ".exp", ".a", ".h", ".hpp"];

/** The llama.cpp archives carry a dozen tools next to the server, and since
 *  b10146 each tool is a pair: a launcher (`llama-cli`, `llama-cli.exe`) plus
 *  the library that holds it (`llama-cli-impl.dll`, `libllama-cli-impl.so`).
 *  Both halves of a dropped tool go; both halves of the server stay.
 *
 *  Reducing a file to its tool name has to account for the library shape, or
 *  the rule keeps `llama-server.exe` — a 9 KB stub — and drops the 9.4 MB
 *  `llama-server-impl.dll` that is the actual server. That shipped: the bundled
 *  binary died at load with 0xC0000139 (STATUS_ENTRYPOINT_NOT_FOUND) because
 *  its only import was a DLL that had been pruned. Linux escaped it by
 *  accident — `libllama-server-impl.so` does not start with `llama-`.
 *
 *  Anything that is not one of those two shapes is a shared library the server
 *  may load, and is kept: guessing which `.dll`/`.so` it needs is what failed
 *  here in the first place. */
export function shouldPrune(fileName) {
  const file = basename(fileName);
  if (BUILD_ONLY_EXTENSIONS.some((ext) => file.toLowerCase().endsWith(ext))) return true;

  // `libfoo.so.0.0.10146` is as much a library as `foo.dll` — matching only a
  // trailing `.so` would let a versioned name fall through to the tool branch.
  const isLibrary = /\.(dll|dylib)$/i.test(file) || /\.so(\.\d+)*$/i.test(file);
  const impl = file.match(/^(?:lib)?(.+)-impl\.(?:dll|so)$/i);
  const tool = impl ? impl[1] : isLibrary ? null : file.replace(/\.exe$/i, "");
  if (!tool) return false;
  return tool.startsWith("llama-") && tool !== "llama-server";
}

export function stampFor(manifest, platform) {
  const stamp = { platform };
  for (const target of Object.keys(LAYOUT)) {
    stamp[target] = assetFor(manifest, target, platform).name;
  }
  return stamp;
}

export function isStampCurrent(existing, expected) {
  if (!existing) return false;
  const keys = Object.keys(expected);
  return keys.every((k) => existing[k] === expected[k]);
}

async function download(url, destination) {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok || !response.body) {
    throw new Error(`GET ${url} answered ${response.status}`);
  }
  await pipeline(Readable.fromWeb(response.body), createWriteStream(destination));
}

/** Dispatched on extension rather than handed to one tool, because `tar` is
 *  not the same program everywhere: bsdtar (Windows 10+) reads .zip, GNU tar
 *  (MSYS, Linux) does not, and which one answers depends on the shell that
 *  invoked npm. Measured here: from Git Bash, `tar -xf` on a .zip fails with
 *  "This does not look like a tar archive".
 *
 *  Only Windows assets are zipped; every Linux asset is a .tar.gz. */
export function extractorFor(fileName) {
  if (/\.zip$/i.test(fileName)) return "zip";
  if (/\.(tar\.gz|tgz)$/i.test(fileName)) return "tar";
  throw new Error(`no extractor for ${fileName}`);
}

function extract(archive, destination) {
  mkdirSync(destination, { recursive: true });
  if (extractorFor(archive) === "zip") {
    // ZipFile beats Expand-Archive by a wide margin on the ~79 MB ONNX
    // Runtime package, and needs no module import.
    execFileSync(
      "powershell",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        // No overwrite flag: Windows PowerShell 5.1 ships .NET Framework,
        // whose only 3-argument overload takes an encoding — passing $true
        // there fails with a cast error. The destination is wiped by the
        // caller before every extraction, so there is nothing to overwrite.
        `Add-Type -AssemblyName System.IO.Compression.FileSystem; ` +
          `[System.IO.Compression.ZipFile]::ExtractToDirectory('${archive}', '${destination}')`,
      ],
      { stdio: "inherit" },
    );
    return;
  }
  // Both tars choke on absolute Windows paths, in different places: bsdtar
  // reads `D:\...` after `-f` as a remote `host:path`, and MSYS GNU tar mangles
  // it after `-C`. Running from the destination with a relative, forward-slash
  // archive path keeps every argument colon-free.
  const from = relative(destination, archive).split(sep).join("/");
  execFileSync("tar", ["-xzf", from], { cwd: destination, stdio: "inherit" });
}

function pruneTree(dir) {
  let removed = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      removed += pruneTree(path);
    } else if (shouldPrune(entry.name)) {
      unlinkSync(path);
      removed += 1;
    }
  }
  return removed;
}

function treeSize(dir) {
  let total = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    total += entry.isDirectory() ? treeSize(path) : statSync(path).size;
  }
  return total;
}

async function main() {
  const force = process.argv.includes("--force");
  const platform = hostPlatform();
  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
  const expected = stampFor(manifest, platform);

  const stampPath = join(RESOURCES, STAMP_FILE);
  if (!force && existsSync(stampPath)) {
    const existing = JSON.parse(readFileSync(stampPath, "utf8"));
    if (isStampCurrent(existing, expected)) {
      console.log("vendor: components already match vendor.json — nothing to do");
      return;
    }
  }

  mkdirSync(RESOURCES, { recursive: true });

  for (const target of Object.keys(LAYOUT)) {
    const asset = assetFor(manifest, target, platform);
    const destination = join(RESOURCES, ...target.split("/"));
    const archive = join(RESOURCES, asset.name);

    console.log(`vendor: ${target} <- ${asset.name}`);
    rmSync(destination, { recursive: true, force: true });
    await download(asset.url, archive);
    extract(archive, destination);
    rmSync(archive, { force: true });

    const pruned = pruneTree(destination);
    const mb = (treeSize(destination) / 1e6).toFixed(1);
    console.log(`vendor: ${target} ready — ${mb} MB, ${pruned} extra tool(s) removed`);
  }

  writeFileSync(stampPath, `${JSON.stringify(expected, null, 2)}\n`);
  console.log(`vendor: total ${(treeSize(RESOURCES) / 1e6).toFixed(1)} MB in ${RESOURCES}`);
}

// Only run when invoked directly, so the test file can import the pure parts.
if (process.argv[1] && process.argv[1].endsWith("vendor-runtime.mjs")) {
  main().catch((error) => {
    console.error(`vendor: ${error.message}`);
    process.exit(1);
  });
}
