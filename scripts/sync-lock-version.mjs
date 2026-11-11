import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function syncLockVersion(lockfile, version) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid release version: ${version}`);
  }

  const entry = /^(\[\[package\]\]\r?\nname = "perfview"\r?\nversion = ")[^"]+(")/gm;
  if ([...lockfile.matchAll(entry)].length !== 1) {
    throw new Error("Expected exactly one perfview package in Cargo.lock");
  }
  return lockfile.replace(entry, (_, prefix, suffix) => `${prefix}${version}${suffix}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const lockfile = readFileSync("Cargo.lock", "utf8");
  writeFileSync("Cargo.lock", syncLockVersion(lockfile, process.argv[2]));
}
