import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import * as XLSX from "xlsx";

const root = path.resolve(new URL("../..", import.meta.url).pathname);
const mynetdiaryDir = path.join(root, "modules/sources/mynetdiary/tests/fixtures");
const hevyDir = path.join(root, "modules/sources/hevy/tests/fixtures");
const deniedText = ["simarglok", "john doe", "real export", "icloud"];

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function scanText(label, text) {
  const lower = text.toLowerCase();
  for (const term of deniedText) assert(!lower.includes(term), `${label} contains denied privacy term`);
  assert(!/[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/i.test(text), `${label} contains an email address`);
}

function verifyManifest(directory, manifestName, requiredFormat, scanRaw = true) {
  const manifestPath = path.join(directory, manifestName);
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  assert(manifest.format === requiredFormat, `${manifestName} format mismatch`);
  assert(Array.isArray(manifest.files) && manifest.files.length > 0, `${manifestName} has no files`);
  for (const entry of manifest.files) {
    const file = path.join(directory, entry.file);
    assert(fs.existsSync(file), `missing fixture ${entry.file}`);
    assert(sha256(file) === entry.sha256, `digest mismatch for ${entry.file}`);
    if (scanRaw) scanText(entry.file, fs.readFileSync(file).toString("utf8"));
  }
  return manifest.files;
}

const xlsFiles = verifyManifest(mynetdiaryDir, "fixture-manifest.json", "BIFF8/CDFV2", false);
for (const entry of xlsFiles) {
  const file = path.join(mynetdiaryDir, entry.file);
  const bytes = fs.readFileSync(file);
  assert(bytes.subarray(0, 8).equals(Buffer.from("d0cf11e0a1b11ae1", "hex")), `${entry.file} is not CDFV2/BIFF8`);
  const workbook = XLSX.read(bytes, { type: "buffer", cellText: true, cellDates: false });
  assert(workbook.SheetNames.length > 0, `${entry.file} has no sheets`);
  for (const sheetName of workbook.SheetNames) {
    const rows = XLSX.utils.sheet_to_json(workbook.Sheets[sheetName], { header: 1, raw: false });
    scanText(`${entry.file}:${sheetName}`, JSON.stringify(rows));
  }
}

const csvFiles = verifyManifest(hevyDir, "fixture-manifest.json", "UTF-8 comma CSV");
for (const entry of csvFiles) {
  const bytes = fs.readFileSync(path.join(hevyDir, entry.file));
  const text = bytes.toString("utf8");
  assert(!text.includes("\ufffd"), `${entry.file} is not valid UTF-8`);
  assert(text.includes(","), `${entry.file} is not comma-separated`);
}

console.log(`verified ${xlsFiles.length} BIFF fixtures and ${csvFiles.length} CSV fixtures; privacy scan passed`);
