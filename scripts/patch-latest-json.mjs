#!/usr/bin/env node
// SPEC: release-distribution (REL-10, REL-12)
//
// Adds the portable entry to the updater manifest that tauri-action published.
//
// tauri-action only knows about the formats it bundles (nsis/msi/appimage), so
// the portable zip has to be added afterwards. `platforms` is a plain map, so an
// extra key is inert for tauri-plugin-updater and readable by our own code.
//
// Usage:
//   node scripts/patch-latest-json.mjs \
//     --manifest latest.json \
//     --key windows-x86_64-portable \
//     --tag v1.0.0 \
//     --signature-file LocalMind_1.0.0_x64-portable.zip.sig \
//     (--url <url> | --assets assets.json --match portable.zip)

import { readFileSync, writeFileSync } from "node:fs";

/**
 * Picks an asset URL by substring instead of rebuilding the filename. Tauri's
 * artifact naming varies between versions and locales (`_x64-setup.exe`,
 * `_x64_en-US.msi`), so guessing it is a latent break.
 */
export function pickAssetUrl(assets, match) {
  if (!Array.isArray(assets)) throw new Error("assets must be an array");
  const hits = assets.filter((asset) => String(asset?.name ?? "").includes(match));
  if (hits.length === 0) throw new Error(`no release asset matching ${JSON.stringify(match)}`);
  if (hits.length > 1) {
    const names = hits.map((a) => a.name).join(", ");
    throw new Error(`ambiguous match ${JSON.stringify(match)}: ${names}`);
  }
  const url = hits[0].url ?? hits[0].browser_download_url;
  if (!url) throw new Error(`asset ${hits[0].name} has no download url`);
  return url;
}

/**
 * Exact-name lookup. The workflow uses this rather than the substring form,
 * because every archive name is also a prefix of its own `.sig` — matching
 * "…_x64-portable.zip" by substring would always be ambiguous in a real release.
 */
export function pickAssetUrlByName(assets, name) {
  if (!Array.isArray(assets)) throw new Error("assets must be an array");
  const hit = assets.find((asset) => String(asset?.name ?? "") === name);
  if (!hit) throw new Error(`no release asset named ${JSON.stringify(name)}`);
  const url = hit.url ?? hit.browser_download_url;
  if (!url) throw new Error(`asset ${name} has no download url`);
  return url;
}

/** `https://github.com/<owner>/<repo>/releases/download/<ref>/<file>` split so
 *  the ref can be replaced without touching owner, repo or filename. */
const DOWNLOAD_URL = /^(https:\/\/github\.com\/[^/]+\/[^/]+\/releases\/download\/)([^/]+)(\/[^/]+)$/;

/**
 * Rewrites the release ref of a GitHub download URL to `tag`.
 *
 * The portable zip is uploaded while the release is still a draft, and a draft
 * has no tag ref: GitHub serves its assets from an ephemeral
 * `/releases/download/untagged-<hash>/` path that stops existing the moment the
 * release is published. Reading that URL and writing it straight into
 * `latest.json` is what shipped v0.2.0 with a portable update pointing at a
 * dead link — measured against the published release: the untagged URL answers
 * HTTP 404, the tagged one 200.
 *
 * Publishing before patching would fix it too, but it would give up the
 * invariant this workflow is built on — the release stays a draft until every
 * artifact is in place. So the asset is still *read* from the release, which is
 * what proves it exists; only the ref segment is corrected.
 */
export function retagDownloadUrl(url, tag) {
  const ref = String(tag ?? "");
  if (!ref || ref.includes("/")) throw new Error(`invalid tag: ${JSON.stringify(tag)}`);

  const parts = DOWNLOAD_URL.exec(String(url ?? ""));
  if (!parts) throw new Error(`not a GitHub release download URL: ${JSON.stringify(url)}`);

  return `${parts[1]}${ref}${parts[3]}`;
}

export function addPlatform(manifest, key, entry) {
  if (!manifest || typeof manifest !== "object") throw new Error("manifest must be an object");
  if (!manifest.platforms || typeof manifest.platforms !== "object") {
    throw new Error("manifest has no `platforms` object — is this a Tauri latest.json?");
  }
  if (!entry?.url) throw new Error("platform entry needs a url");
  if (!entry?.signature) throw new Error("platform entry needs a signature");

  return {
    ...manifest,
    platforms: { ...manifest.platforms, [key]: { signature: entry.signature, url: entry.url } },
  };
}

function parseArgs(argv) {
  const flags = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) continue;
    flags[arg.slice(2)] = argv[++i];
  }
  return flags;
}

function main(argv) {
  const flags = parseArgs(argv);
  const manifestPath = flags.manifest ?? "latest.json";
  const key = flags.key ?? "windows-x86_64-portable";

  if (!flags["signature-file"]) throw new Error("--signature-file is required");
  const signature = readFileSync(flags["signature-file"], "utf8").trim();

  if (!flags.tag) throw new Error("--tag is required");

  let url = flags.url;
  if (!url) {
    if (!flags.assets || !(flags.name || flags.match)) {
      throw new Error("either --url, or --assets together with --name/--match, is required");
    }
    const assets = JSON.parse(readFileSync(flags.assets, "utf8"));
    url = flags.name ? pickAssetUrlByName(assets, flags.name) : pickAssetUrl(assets, flags.match);
  }
  // Applies to `--url` too: one normalisation path means the guarantee cannot
  // be lost by picking the other input.
  url = retagDownloadUrl(url, flags.tag);

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const patched = addPlatform(manifest, key, { url, signature });
  writeFileSync(manifestPath, `${JSON.stringify(patched, null, 2)}\n`);

  process.stdout.write(`${key} -> ${url}\n`);
}

if (process.argv[1] && process.argv[1].endsWith("patch-latest-json.mjs")) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
