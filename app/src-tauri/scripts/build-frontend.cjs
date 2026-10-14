
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const frontendDir = path.resolve(__dirname, "../../web");
const packageJson = path.join(frontendDir, "package.json");

if (!fs.existsSync(packageJson)) {
  console.error(`Frontend package.json not found: ${packageJson}`);
  process.exit(1);
}

console.log(`Building frontend: ${frontendDir}`);

const result = spawnSync("npm", ["run", "build"], {
  cwd: frontendDir,
  stdio: "inherit",
  shell: process.platform === "win32",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);

