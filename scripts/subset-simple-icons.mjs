import { readFileSync, writeFileSync } from 'fs';

const iconsJson = JSON.parse(
  readFileSync('node_modules/@iconify-json/simple-icons/icons.json', 'utf-8')
);

const keep = [
  // Currently used
  'openrouter', 'openai', 'anthropic', 'github',
  // AI providers
  'google', 'googlegemini', 'meta', 'mistral', 'deepseek',
  // Social / professional
  'linkedin', 'x', 'twitter', 'discord', 'slack', 'reddit',
  'youtube', 'twitch', 'instagram', 'facebook', 'tiktok',
  // Dev platforms
  'gitlab', 'bitbucket', 'stackoverflow', 'npm', 'docker',
  'kubernetes', 'vercel', 'netlify', 'cloudflare', 'amazonaws',
  'googlecloud', 'microsoftazure', 'digitalocean', 'heroku',
  'supabase', 'firebase', 'mongodb', 'postgresql', 'mysql',
  'redis', 'sqlite', 'prisma', 'graphql',
  // Languages / frameworks
  'rust', 'go', 'python', 'javascript', 'typescript',
  'react', 'svelte', 'vuedotjs', 'angular', 'nextdotjs',
  'nodedotjs', 'deno', 'bun', 'tailwindcss',
  // Tools
  'git', 'visualstudiocode', 'neovim', 'figma',
  'apple', 'windows', 'linux',
  // Payment / business
  'stripe', 'shopify',
];

const subset = {
  prefix: iconsJson.prefix,
  icons: {},
  aliases: {},
  width: iconsJson.width,
  height: iconsJson.height,
};

let found = 0;
for (const name of keep) {
  if (iconsJson.icons[name]) {
    subset.icons[name] = iconsJson.icons[name];
    found++;
  } else if (iconsJson.aliases?.[name]) {
    subset.aliases[name] = iconsJson.aliases[name];
    const parent = iconsJson.aliases[name].parent;
    if (parent && iconsJson.icons[parent]) subset.icons[parent] = iconsJson.icons[parent];
    found++;
  }
}

const output = JSON.stringify(subset);
writeFileSync('src/lib/icons/simple-icons-subset.json', output);

console.log(`✓ Subset: ${found}/${keep.length} icons found`);
console.log(`  Full: ${(JSON.stringify(iconsJson).length / 1024).toFixed(0)} KB`);
console.log(`  Subset: ${(output.length / 1024).toFixed(0)} KB`);
console.log(`  Savings: ${((1 - output.length / JSON.stringify(iconsJson).length) * 100).toFixed(0)}%`);
const missing = keep.filter(n => !iconsJson.icons[n] && !iconsJson.aliases?.[n]);
if (missing.length) console.log(`  Missing:`, missing);
