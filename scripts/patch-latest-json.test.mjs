import assert from "node:assert/strict";
import { test } from "node:test";

import { addPlatform, pickAssetUrl, pickAssetUrlByName } from "./patch-latest-json.mjs";

const manifest = () => ({
  version: "1.0.0",
  notes: "release notes",
  pub_date: "2026-07-26T12:00:00Z",
  platforms: {
    "windows-x86_64-nsis": { signature: "sig-nsis", url: "https://example/setup.exe" },
    "linux-x86_64-appimage": { signature: "sig-appimage", url: "https://example/app.AppImage" },
  },
});

test("addPlatform adds the portable key without dropping the existing ones", () => {
  const out = addPlatform(manifest(), "windows-x86_64-portable", {
    url: "https://example/portable.zip",
    signature: "sig-portable",
  });

  assert.deepEqual(Object.keys(out.platforms).sort(), [
    "linux-x86_64-appimage",
    "windows-x86_64-nsis",
    "windows-x86_64-portable",
  ]);
  assert.deepEqual(out.platforms["windows-x86_64-nsis"], {
    signature: "sig-nsis",
    url: "https://example/setup.exe",
  });
  assert.deepEqual(out.platforms["windows-x86_64-portable"], {
    signature: "sig-portable",
    url: "https://example/portable.zip",
  });
});

test("addPlatform preserves the top-level fields", () => {
  const out = addPlatform(manifest(), "windows-x86_64-portable", { url: "u", signature: "s" });
  assert.equal(out.version, "1.0.0");
  assert.equal(out.notes, "release notes");
  assert.equal(out.pub_date, "2026-07-26T12:00:00Z");
});

test("addPlatform does not mutate its input", () => {
  const original = manifest();
  addPlatform(original, "windows-x86_64-portable", { url: "u", signature: "s" });
  assert.equal(Object.keys(original.platforms).length, 2);
});

test("addPlatform rejects a manifest without platforms", () => {
  assert.throws(() => addPlatform({ version: "1.0.0" }, "k", { url: "u", signature: "s" }), /no `platforms`/);
  assert.throws(() => addPlatform(null, "k", { url: "u", signature: "s" }), /must be an object/);
});

test("addPlatform requires both url and signature", () => {
  assert.throws(() => addPlatform(manifest(), "k", { signature: "s" }), /needs a url/);
  assert.throws(() => addPlatform(manifest(), "k", { url: "u" }), /needs a signature/);
});

test("pickAssetUrl finds the asset by substring", () => {
  const assets = [
    { name: "LocalMind_1.0.0_x64-setup.exe", browser_download_url: "https://example/setup.exe" },
    { name: "LocalMind_1.0.0_x64_en-US.msi", browser_download_url: "https://example/app.msi" },
    { name: "LocalMind_1.0.0_x64-portable.zip", browser_download_url: "https://example/portable.zip" },
  ];
  assert.equal(pickAssetUrl(assets, "x64-portable.zip"), "https://example/portable.zip");
  assert.equal(pickAssetUrl(assets, "en-US.msi"), "https://example/app.msi");
});

test("pickAssetUrl is strict about ambiguity", () => {
  const assets = [
    { name: "LocalMind_1.0.0_x64-portable.zip", browser_download_url: "https://example/portable.zip" },
    { name: "LocalMind_1.0.0_x64-portable.zip.sig", browser_download_url: "https://example/portable.zip.sig" },
  ];
  // "portable.zip" matches both the archive and its signature — refusing beats
  // silently picking the wrong one.
  assert.throws(() => pickAssetUrl(assets, "portable.zip"), /ambiguous match/);
  assert.equal(pickAssetUrl(assets, "portable.zip.sig"), "https://example/portable.zip.sig");
});

test("pickAssetUrl accepts the gh CLI `url` field as well", () => {
  const assets = [{ name: "LocalMind_1.0.0_x64-portable.zip", url: "https://api/assets/1" }];
  assert.equal(pickAssetUrl(assets, "x64-portable.zip"), "https://api/assets/1");
});

test("pickAssetUrl fails when nothing matches", () => {
  assert.throws(() => pickAssetUrl([{ name: "a.exe", url: "u" }], "portable"), /no release asset matching/);
  assert.throws(() => pickAssetUrl(null, "portable"), /must be an array/);
});

test("pickAssetUrlByName is not confused by the .sig sharing the archive name", () => {
  const assets = [
    { name: "LocalMind_1.0.0_x64-portable.zip", browser_download_url: "https://example/portable.zip" },
    { name: "LocalMind_1.0.0_x64-portable.zip.sig", browser_download_url: "https://example/portable.zip.sig" },
  ];
  assert.equal(pickAssetUrlByName(assets, "LocalMind_1.0.0_x64-portable.zip"), "https://example/portable.zip");
  assert.equal(
    pickAssetUrlByName(assets, "LocalMind_1.0.0_x64-portable.zip.sig"),
    "https://example/portable.zip.sig",
  );
});

test("pickAssetUrlByName fails on an unknown name", () => {
  assert.throws(() => pickAssetUrlByName([{ name: "a.exe", url: "u" }], "b.exe"), /no release asset named/);
  assert.throws(() => pickAssetUrlByName(null, "a"), /must be an array/);
});
