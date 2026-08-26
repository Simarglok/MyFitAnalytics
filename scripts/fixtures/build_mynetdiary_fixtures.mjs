import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import * as XLSX from "xlsx";

const root = path.resolve(new URL("../..", import.meta.url).pathname);
const mynetdiaryDir = path.join(root, "modules/sources/mynetdiary/tests/fixtures");
const hevyDir = path.join(root, "modules/sources/hevy/tests/fixtures");
fs.mkdirSync(mynetdiaryDir, { recursive: true });
fs.mkdirSync(hevyDir, { recursive: true });

const foodHeader = [
  "Date", "Time", "Food Name", "Food ID", "Amount", "Calories",
  "Protein, g", "Fat, g", "Carbs, g", "Fiber, g", "Sugars, g", "Sodium, mg", "Notes",
];
const measurementsHeader = ["Date", "Type", "Value", "Unit", "Notes"];
const exerciseHeader = ["Date", "Activity", "Duration, min", "Distance, km", "Calories", "Notes"];
const trackersHeader = ["Date", "Type", "Value", "Unit", "Notes"];
const waterHeader = ["Date", "Water, ml", "Glasses", "Notes"];

const fullSheets = {
  Food: [
    foodHeader,
    ["2026-01-04", "08:15", "Synthetic Oatmeal", "F-001", "1 serving", "320", "12.5", "8.0", "48.0", "6.0", "9.0", "180", "fictional breakfast"],
    ["2026-01-04", "08:15", "Synthetic Oatmeal", "F-001", "1 serving", "320", "12.5", "8.0", "48.0", "6.0", "9.0", "180", "duplicate row remains distinct"],
  ],
  Measurements: [
    measurementsHeader,
    ["2026-01-04", "Weight", "82.5", "kg", "fictional measurement"],
    ["2026-01-04", "Daily Steps Count", "6400", "steps", "fictional steps"],
  ],
  Exercise: [
    exerciseHeader,
    ["2026-01-04", "Walking", "30", "2.4", "140", "fictional walk"],
    ["2026-01-04", "Traditional Strength Training", "25", "", "110", "strength is non-canonical here"],
  ],
  Trackers: [
    trackersHeader,
    ["2026-01-04", "Heart Rate", "128", "bpm", "fictional tracker"],
  ],
  "Water Glasses": [
    waterHeader,
    ["2026-01-04", "500", "2", "fictional water"],
  ],
};

const scenarios = [
  { file: "valid-full.xls", scenario: "all required sheets and optional sheets" },
  { file: "missing-required-sheet.xls", scenario: "Exercise sheet omitted" },
  { file: "optional-sheets-absent.xls", scenario: "Trackers and Water Glasses omitted" },
  { file: "schema-drift.xls", scenario: "required Food column renamed" },
  { file: "mixed-year.xls", scenario: "dates span two calendar years" },
  { file: "unknown-activity.xls", scenario: "governed activity name is unknown" },
  { file: "decimal-comma-nbsp.xls", scenario: "decimal comma and NBSP numeric text" },
];

function cloneSheets(sheets) {
  return Object.fromEntries(Object.entries(sheets).map(([name, rows]) => [name, rows.map((row) => [...row])]));
}

function writeWorkbook(file, sheets) {
  const workbook = XLSX.utils.book_new();
  workbook.Props = { Title: "Synthetic MyFitAnalytics fixture", Subject: "Synthetic test data", CreatedDate: new Date(0) };
  for (const [name, rows] of Object.entries(sheets)) {
    XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet(rows), name);
  }
  XLSX.writeFile(workbook, file, { bookType: "biff8", compression: false });
}

function writeCsv(file, rows) {
  const text = rows.map((row) => row.map((cell) => String(cell ?? "")).join(",")).join("\n") + "\n";
  fs.writeFileSync(file, text, "utf8");
}

function makeScenario(name) {
  const sheets = cloneSheets(fullSheets);
  if (name === "missing-required-sheet.xls") delete sheets.Exercise;
  if (name === "optional-sheets-absent.xls") {
    delete sheets.Trackers;
    delete sheets["Water Glasses"];
  }
  if (name === "schema-drift.xls") sheets.Food[0][5] = "Calories (renamed)";
  if (name === "mixed-year.xls") sheets.Measurements[2][0] = "2025-12-31";
  if (name === "unknown-activity.xls") sheets.Exercise[1][1] = "Synthetic Unknown Activity";
  if (name === "decimal-comma-nbsp.xls") {
    sheets.Food[1][5] = "1\u00a0234,5";
    sheets.Food[1][6] = "12,5";
    sheets.Exercise[1][2] = "25,5";
  }
  return sheets;
}

for (const { file } of scenarios) writeWorkbook(path.join(mynetdiaryDir, file), makeScenario(file));

writeCsv(path.join(hevyDir, "measurement_data.csv"), [
  ["date", "weight_kg", "fat_percent", "waist_cm", "neck_cm", "hip_cm"],
  ["2026-02-01 00:00:00", "81.4", "18.2", "86.0", "38.0", "98.0"],
  ["2026-02-02", "", "", "", "", ""],
  ["2026-02-02", "81.1", "", "85.5", "", "97.5"],
].map((row) => row.map((value) => value.replaceAll("\u00a0", " "))));
writeCsv(path.join(hevyDir, "workout_data.csv"), [
  ["title", "start_time", "end_time", "exercise_title", "set_index", "set_type", "weight_kg", "reps", "rpe", "duration_seconds", "notes"],
  ["Synthetic Push", "2026-02-03 17:00:00", "2026-02-03 17:42:00", "Bench Press", "1", "normal", "60", "8", "8", "", "fictional set"],
  ["Synthetic Push", "2026-02-03 17:00:00", "2026-02-03 17:42:00", "Bench Press", "2", "warmup", "40", "10", "", "", ""],
  ["Synthetic Push", "2026-02-03 17:00:00", "2026-02-03 17:42:00", "Plank", "1", "normal", "", "", "", "45", "duration set"],
  ["Synthetic Push", "2026-02-03 17:00:00", "2026-02-03 17:42:00", "Bench Press", "3", "failure", "62.5", "6", "10", "", "repeated title is a new block"],
].map((row) => row.map((value) => value.replaceAll("\u00a0", " "))));

const files = scenarios.map(({ file, scenario }) => {
  const bytes = fs.readFileSync(path.join(mynetdiaryDir, file));
  return { file, scenario, sha256: crypto.createHash("sha256").update(bytes).digest("hex") };
});
const csvFiles = ["measurement_data.csv", "workout_data.csv"].map((file) => {
  const bytes = fs.readFileSync(path.join(hevyDir, file));
  return { file, sha256: crypto.createHash("sha256").update(bytes).digest("hex") };
});
fs.writeFileSync(
  path.join(mynetdiaryDir, "fixture-manifest.json"),
  `${JSON.stringify({ generator: "synthetic-mynetdiary-fixtures", format: "BIFF8/CDFV2", files }, null, 2)}\n`,
);
fs.writeFileSync(
  path.join(hevyDir, "fixture-manifest.json"),
  `${JSON.stringify({ generator: "synthetic-hevy-fixtures", format: "UTF-8 comma CSV", files: csvFiles }, null, 2)}\n`,
);
console.log(`wrote ${files.length} BIFF fixtures and ${csvFiles.length} Hevy CSV fixtures`);
