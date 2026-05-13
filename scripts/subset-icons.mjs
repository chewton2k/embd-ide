/**
 * Generate a subset of @iconify-json/vscode-icons containing only the icons
 * actually referenced in fileIcons.ts. This reduces the bundle from ~3.6 MB
 * to ~300 KB.
 *
 * Run: node scripts/subset-icons.mjs
 * Output: src/lib/icons/vscode-icons-subset.json
 */
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { execSync } from 'child_process';

const iconsJson = JSON.parse(
  readFileSync('node_modules/@iconify-json/vscode-icons/icons.json', 'utf-8')
);

// Extract icon names used in fileIcons.ts (strip the "vscode-icons:" prefix)
const raw = execSync(
  `grep -o "vscode-icons:[a-z0-9-]*" src/lib/modules/explorer/fileIcons.ts | sort -u`
).toString().trim().split('\n');

const needed = new Set(raw.map(s => s.replace('vscode-icons:', '')));

// Also include "-opened" variants for folder icons
for (const name of [...needed]) {
  if (name.startsWith('folder-type-')) {
    needed.add(name + '-opened');
  }
}

const subset = {
  prefix: iconsJson.prefix,
  icons: {},
  aliases: {},
  width: iconsJson.width,
  height: iconsJson.height,
};

let found = 0;
let missing = [];

for (const name of needed) {
  if (iconsJson.icons[name]) {
    subset.icons[name] = iconsJson.icons[name];
    found++;
  } else if (iconsJson.aliases && iconsJson.aliases[name]) {
    subset.aliases[name] = iconsJson.aliases[name];
    // Also include the parent icon
    const parent = iconsJson.aliases[name].parent;
    if (parent && iconsJson.icons[parent]) {
      subset.icons[parent] = iconsJson.icons[parent];
    }
    found++;
  } else {
    missing.push(name);
  }
}

mkdirSync('src/lib/icons', { recursive: true });
const output = JSON.stringify(subset);
writeFileSync('src/lib/icons/vscode-icons-subset.json', output);

console.log(`✓ Subset: ${found} icons (${missing.length} not found)`);
console.log(`  Full: ${(JSON.stringify(iconsJson).length / 1024).toFixed(0)} KB`);
console.log(`  Subset: ${(output.length / 1024).toFixed(0)} KB`);
console.log(`  Savings: ${((1 - output.length / JSON.stringify(iconsJson).length) * 100).toFixed(0)}%`);
if (missing.length) console.log(`  Missing:`, missing);
