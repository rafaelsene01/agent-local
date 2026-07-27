import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  LAYOUT,
  assetFor,
  extractorFor,
  hostPlatform,
  isStampCurrent,
  shouldPrune,
  stampFor,
} from "./vendor-runtime.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(join(ROOT, "scripts", "vendor.json"), "utf8"));

test("each target resolves to the asset written out in vendor.json", () => {
  const vulkan = assetFor(manifest, "llama/vulkan", "win32");
  assert.equal(vulkan.name, "llama-b10146-bin-win-vulkan-x64.zip");
  assert.match(vulkan.url, /^https:\/\/github\.com\/ggml-org\/llama\.cpp\/releases\/download\//);

  const cpu = assetFor(manifest, "llama/cpu", "linux");
  assert.equal(cpu.name, "llama-b10146-bin-ubuntu-x64.tar.gz");

  // The two llama.cpp backends must never resolve to the same file: shipping
  // the Vulkan build twice would leave machines with no loader unbootable.
  assert.notEqual(
    assetFor(manifest, "llama/vulkan", "linux").name,
    assetFor(manifest, "llama/cpu", "linux").name,
  );
});

test("every target has an asset on both supported platforms", () => {
  for (const platform of ["win32", "linux"]) {
    for (const target of Object.keys(LAYOUT)) {
      const asset = assetFor(manifest, target, platform);
      assert.ok(asset.name.length > 0, `${target} on ${platform}`);
    }
  }
});

test("a missing asset fails naming what was looked for, instead of guessing", () => {
  const incomplete = { llamaCpp: { repo: "x/y", tag: "b1", assets: { win32: {} } } };
  assert.throws(
    () => assetFor(incomplete, "llama/vulkan", "win32"),
    /llamaCpp\.vulkan on win32/,
  );
  assert.throws(() => assetFor({}, "pdfium", "linux"), /no "pdfium" entry/);
});

test("macOS is refused up front rather than downloading nothing usable", () => {
  assert.equal(hostPlatform("win32"), "win32");
  assert.equal(hostPlatform("linux"), "linux");
  assert.throws(() => hostPlatform("darwin"), /unsupported platform: darwin/);
});

test("pruning drops the other llama tools and keeps every shared library", () => {
  for (const name of ["llama-cli.exe", "llama-bench", "llama-quantize.exe", "llama-perplexity"]) {
    assert.equal(shouldPrune(name), true, name);
  }
  for (const name of [
    "llama-server.exe",
    "llama-server",
    "llama.dll",
    "libllama.so",
    "libllama-common.so.0.0.10146",
    "ggml-vulkan.dll",
    "libggml-base.so",
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "libonnxruntime.so",
    "pdfium.dll",
    "libpdfium.so",
  ]) {
    assert.equal(shouldPrune(name), false, name);
  }
});

/// The regression that the case list above walked straight past: every library
/// it named avoided the one combination that breaks — the `llama-` prefix on a
/// `.dll`. `llama-server.exe` is a 9 KB launcher and `llama-server-impl.dll` is
/// the 9.4 MB server; pruning the second shipped a binary that cannot start
/// (0xC0000139, verified by running it). Linux was spared only because its copy
/// is called `libllama-server-impl.so`, which the old rule never matched.
test("the server's implementation library survives, its siblings' do not", () => {
  for (const name of ["llama-server-impl.dll", "libllama-server-impl.so"]) {
    assert.equal(shouldPrune(name), false, name);
  }
  for (const name of [
    "llama-cli-impl.dll",
    "llama-bench-impl.dll",
    "llama-batched-bench-impl.dll",
    "libllama-cli-impl.so",
    "libllama-perplexity-impl.so",
  ]) {
    assert.equal(shouldPrune(name), true, name);
  }
});

/// Measured, not guessed: the ONNX Runtime Windows package extracts to 426 MB
/// and `onnxruntime.pdb` alone is 408 MB of it. Dropping build-only files takes
/// that component to 16.2 MB and the whole vendored tree to 120.5 MB.
test("pruning drops debug symbols and link-time artefacts", () => {
  for (const name of [
    "onnxruntime.pdb",
    "onnxruntime.lib",
    "onnxruntime_providers_shared.exp",
    "libpdfium.a",
    "onnxruntime_c_api.h",
    "cpp/onnxruntime_cxx_api.hpp",
  ]) {
    assert.equal(shouldPrune(name), true, name);
  }
});

/// One tool cannot read both formats here: GNU tar (MSYS/Linux) refuses .zip,
/// and which `tar` answers depends on the shell that started npm.
test("the extractor is chosen by extension, and an unknown one is an error", () => {
  assert.equal(extractorFor("llama-b10146-bin-win-vulkan-x64.zip"), "zip");
  assert.equal(extractorFor("onnxruntime-win-x64-1.28.0.zip"), "zip");
  assert.equal(extractorFor("llama-b10146-bin-ubuntu-x64.tar.gz"), "tar");
  assert.equal(extractorFor("pdfium-win-x64.tgz"), "tar");
  assert.throws(() => extractorFor("something.7z"), /no extractor for something\.7z/);
});

test("every pinned asset is in a format the script can actually open", () => {
  for (const platform of ["win32", "linux"]) {
    for (const target of Object.keys(LAYOUT)) {
      const { name } = assetFor(manifest, target, platform);
      assert.doesNotThrow(() => extractorFor(name), `${target} on ${platform}: ${name}`);
    }
  }
});

test("the stamp covers every target so a partial vendor is never taken as done", () => {
  const stamp = stampFor(manifest, "win32");
  for (const target of Object.keys(LAYOUT)) {
    assert.ok(stamp[target], `stamp is missing ${target}`);
  }
  assert.equal(stamp.platform, "win32");
});

test("a stamp is only current when every pinned name still matches", () => {
  const expected = stampFor(manifest, "linux");
  assert.equal(isStampCurrent({ ...expected }, expected), true);
  assert.equal(isStampCurrent(null, expected), false);
  assert.equal(
    isStampCurrent({ ...expected, "llama/vulkan": "llama-b9999-bin-ubuntu-vulkan-x64.tar.gz" }, expected),
    false,
    "bumping the pinned tag has to re-download",
  );
  // A stamp written on another platform must not satisfy this one.
  assert.equal(isStampCurrent(stampFor(manifest, "win32"), expected), false);
});
