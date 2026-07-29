import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fixtureSecrets } from "./security-fixtures.mjs";

const repoRoot = resolve(new URL("..", import.meta.url).pathname);
const scanRoot = resolve(
  repoRoot,
  process.env.POSTMITE_SECURITY_SCAN_ROOT ?? "target/postmite-security-e2e",
);

if (!existsSync(scanRoot)) {
  console.log(`fixture secret scan skipped; ${scanRoot} does not exist`);
  process.exit(0);
}

const findings = [];
for (const file of walk(scanRoot)) {
  const content = readFileSync(file);
  for (const fixture of fixtureSecrets) {
    if (content.includes(Buffer.from(fixture.value))) {
      findings.push(`${file}: contains ${fixture.label}`);
    }
  }
}

if (findings.length > 0) {
  console.error("Fixture Secret scan failed:");
  for (const finding of findings) {
    console.error(`- ${finding}`);
  }
  process.exit(1);
}

console.log(`fixture secret scan passed: ${scanRoot}`);

function* walk(root) {
  const entries = readdirSync(root).sort();
  for (const entry of entries) {
    const path = resolve(root, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      yield* walk(path);
    } else if (stat.isFile()) {
      yield path;
    }
  }
}
