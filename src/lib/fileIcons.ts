const specialFileIcons: Record<string, string> = {
  'package.json': 'vscode-icons:file-type-package',
  'package-lock.json': 'vscode-icons:file-type-node',
  'cargo.toml': 'vscode-icons:file-type-cargo',
  'dockerfile': 'vscode-icons:file-type-docker',
  '.gitignore': 'vscode-icons:file-type-git',
  '.gitmodules': 'vscode-icons:file-type-git',
  '.env': 'vscode-icons:file-type-dotenv',
};

const extensionIcons: Record<string, string> = {
  ts: 'vscode-icons:file-type-typescript',
  tsx: 'vscode-icons:file-type-reactts',
  js: 'vscode-icons:file-type-javascript',
  jsx: 'vscode-icons:file-type-reactjs',
  py: 'vscode-icons:file-type-python',
  rs: 'vscode-icons:file-type-rust',
  go: 'vscode-icons:file-type-go',
  html: 'vscode-icons:file-type-html',
  svelte: 'vscode-icons:file-type-svelte',
  vue: 'vscode-icons:file-type-vue',
  css: 'vscode-icons:file-type-css',
  scss: 'vscode-icons:file-type-scss',
  less: 'vscode-icons:file-type-less',
  json: 'vscode-icons:file-type-json',
  md: 'vscode-icons:file-type-markdown',
  mdx: 'vscode-icons:file-type-markdown',
  markdown: 'vscode-icons:file-type-markdown',
  toml: 'vscode-icons:file-type-toml',
  yml: 'vscode-icons:file-type-yaml',
  yaml: 'vscode-icons:file-type-yaml',
  lock: 'vscode-icons:file-type-config',
  svg: 'vscode-icons:file-type-image',
  png: 'vscode-icons:file-type-image',
  jpg: 'vscode-icons:file-type-image',
  jpeg: 'vscode-icons:file-type-image',
  gif: 'vscode-icons:file-type-image',
  webp: 'vscode-icons:file-type-image',
  ico: 'vscode-icons:file-type-image',
  bmp: 'vscode-icons:file-type-image',
  pdf: 'vscode-icons:file-type-pdf2',
  sh: 'vscode-icons:file-type-shell',
  bash: 'vscode-icons:file-type-shell',
  zsh: 'vscode-icons:file-type-shell',
};

export function getFileIconName(name: string, isDir = false, isOpen = false): string {
  if (isDir) {
    return isOpen ? 'vscode-icons:default-folder-opened' : 'vscode-icons:default-folder';
  }

  const lower = name.toLowerCase();
  if (specialFileIcons[lower]) return specialFileIcons[lower];
  if (lower.startsWith('.env.')) return 'vscode-icons:file-type-dotenv';

  const ext = lower.split('.').pop() ?? '';
  return extensionIcons[ext] ?? 'vscode-icons:default-file';
}
