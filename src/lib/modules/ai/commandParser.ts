export interface CommandProposal {
  id: string;
  command: string;
  description?: string;
  dangerous: boolean;
}

const DANGEROUS_PATTERNS = [
  /rm\s+(-rf?|--recursive)\s/i,
  /rm\s+-[a-z]*f/i,
  /rmdir/i,
  /git\s+push\s+--force/i,
  /git\s+reset\s+--hard/i,
  /git\s+clean\s+-f/i,
  /drop\s+(table|database)/i,
  /truncate\s+table/i,
  /delete\s+from\s+\w+\s*;?\s*$/i,
  /:(){ :\|:& };:/,
  /mkfs/i,
  /dd\s+if=/i,
  />\s*\/dev\/sd/i,
  /chmod\s+-R\s+777/i,
  /curl.*\|\s*(bash|sh)/i,
];

const RUN_BLOCK_RE = /```run\n([\s\S]*?)```/g;

/** Parse AI response for command proposals. */
export function parseCommands(response: string): { commands: CommandProposal[]; displayText: string } {
  const commands: CommandProposal[] = [];
  let displayText = response;

  let match: RegExpExecArray | null;
  const re = new RegExp(RUN_BLOCK_RE.source, 'g');

  while ((match = re.exec(response)) !== null) {
    const [fullMatch, body] = match;
    const command = body.trim();
    if (!command) continue;

    const dangerous = DANGEROUS_PATTERNS.some(p => p.test(command));

    commands.push({
      id: `cmd-${Date.now()}-${commands.length}`,
      command,
      dangerous,
    });

    const icon = dangerous ? '⚠️' : '▶';
    displayText = displayText.replace(fullMatch, `${icon} \`${command}\``);
  }

  return { commands, displayText };
}

export function hasCommands(response: string): boolean {
  return /```run\n/.test(response);
}
