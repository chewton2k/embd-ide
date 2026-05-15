/**
 * Native tool definitions for the agent loop.
 *
 * Each tool has:
 * - A JSON schema (OpenAI function-calling format)
 * - A dispatch function that executes the tool and returns a string result
 *
 * The schemas are sent to the model via the `tools` parameter.
 * The dispatch function is called when the model returns a tool_call.
 */
import { invoke } from '@tauri-apps/api/core';

// ── Tool Schema Types (OpenAI format) ──

export interface ToolParameter {
  type: string;
  description: string;
  enum?: string[];
}

export interface ToolSchema {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: {
      type: 'object';
      properties: Record<string, ToolParameter>;
      required: string[];
    };
  };
}

export interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string; // JSON string
  };
}

export interface ToolResult {
  tool_call_id: string;
  content: string;
  success: boolean;
}

// ── Tool Definitions ──

export const TOOL_SCHEMAS: ToolSchema[] = [
  {
    type: 'function',
    function: {
      name: 'read_file',
      description: 'Read the contents of a file. Returns the file content as text.',
      parameters: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'File path relative to project root' },
        },
        required: ['path'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'edit_file',
      description: 'Edit a file by replacing a range of lines with new content.',
      parameters: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'File path relative to project root' },
          start_line: { type: 'string', description: '1-indexed start line number' },
          end_line: { type: 'string', description: '1-indexed end line number' },
          new_content: { type: 'string', description: 'The new code to replace the specified lines' },
        },
        required: ['path', 'start_line', 'end_line', 'new_content'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'run_command',
      description: 'Run a shell command and return its output. Use for builds, tests, linting.',
      parameters: {
        type: 'object',
        properties: {
          command: { type: 'string', description: 'The shell command to execute' },
        },
        required: ['command'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'search_files',
      description: 'Search for files by name pattern in the project.',
      parameters: {
        type: 'object',
        properties: {
          query: { type: 'string', description: 'Search query (matches against file paths)' },
        },
        required: ['query'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'grep',
      description: 'Search for a text pattern across project files. Returns matching lines with file paths.',
      parameters: {
        type: 'object',
        properties: {
          pattern: { type: 'string', description: 'Text or regex pattern to search for' },
          path: { type: 'string', description: 'Optional subdirectory to limit search scope' },
        },
        required: ['pattern'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'list_dir',
      description: 'List files and directories at a given path.',
      parameters: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'Directory path relative to project root (empty string for root)' },
        },
        required: ['path'],
      },
    },
  },
];

// ── Tool Names (for type safety) ──

export type ToolName = 'read_file' | 'edit_file' | 'run_command' | 'search_files' | 'grep' | 'list_dir';

export const ALL_TOOL_NAMES: ToolName[] = ['read_file', 'edit_file', 'run_command', 'search_files', 'grep', 'list_dir'];

// ── Dispatch ──

export interface DispatchContext {
  projectRoot: string;
  /** Called when an edit is proposed (for pending-edit flow) */
  onEdit?: (path: string, startLine: number, endLine: number, newContent: string, originalCode: string) => void;
  /** Called when a command needs approval */
  onCommandBlocked?: (command: string) => void;
}

/**
 * Parse tool call arguments safely.
 * Models sometimes return malformed JSON — this handles common issues.
 */
export function parseToolArgs(argsJson: string): Record<string, string> {
  try {
    return JSON.parse(argsJson);
  } catch {
    // Try to recover from common model mistakes (trailing commas, etc.)
    try {
      const cleaned = argsJson.replace(/,\s*}/g, '}').replace(/,\s*]/g, ']');
      return JSON.parse(cleaned);
    } catch {
      return {};
    }
  }
}

/**
 * Dispatch a tool call and return the result string.
 */
export async function dispatchTool(
  toolCall: ToolCall,
  ctx: DispatchContext,
): Promise<ToolResult> {
  const name = toolCall.function.name as ToolName;
  const args = parseToolArgs(toolCall.function.arguments);

  try {
    const content = await executeTool(name, args, ctx);
    return { tool_call_id: toolCall.id, content, success: true };
  } catch (e) {
    return { tool_call_id: toolCall.id, content: `Error: ${e}`, success: false };
  }
}

async function executeTool(
  name: ToolName,
  args: Record<string, string>,
  ctx: DispatchContext,
): Promise<string> {
  switch (name) {
    case 'read_file': {
      const path = resolvePath(args.path, ctx.projectRoot);
      const content = await invoke<string>('read_file_content', { path });
      // Truncate large files to avoid blowing context
      const MAX = 8000;
      if (content.length > MAX) {
        return content.slice(0, MAX) + `\n\n... (truncated, ${content.length - MAX} more chars)`;
      }
      return content;
    }

    case 'edit_file': {
      const path = resolvePath(args.path, ctx.projectRoot);
      const startLine = parseInt(args.start_line, 10);
      const endLine = parseInt(args.end_line, 10);
      if (isNaN(startLine) || isNaN(endLine)) {
        throw new Error('start_line and end_line must be numbers');
      }
      // Always read the file to get original code for diff display
      const fileContent = await invoke<string>('read_file_content', { path });
      const fileLines = fileContent.split('\n');
      const originalCode = fileLines.slice(startLine - 1, endLine).join('\n');

      if (ctx.onEdit) {
        ctx.onEdit(path, startLine, endLine, args.new_content, originalCode);
        return `Edit proposed for ${args.path} lines ${startLine}-${endLine}. Waiting for approval.`;
      }
      // Auto-apply mode
      const before = fileLines.slice(0, startLine - 1);
      const after = fileLines.slice(endLine);
      const newContent = [...before, ...args.new_content.split('\n'), ...after].join('\n');
      await invoke('write_file_content', { path, content: newContent });
      return `Applied edit to ${args.path} lines ${startLine}-${endLine}.`;
    }

    case 'run_command': {
      const command = args.command;
      if (!command) throw new Error('command is required');
      // Use the run_sandboxed command if available, otherwise indicate blocked
      try {
        const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>(
          'run_command_capture',
          { command, cwd: ctx.projectRoot, timeoutMs: 30000 }
        );
        let output = '';
        if (result.stdout) output += result.stdout;
        if (result.stderr) output += (output ? '\n' : '') + result.stderr;
        if (!output) output = `(no output, exit code: ${result.exit_code})`;
        // Truncate
        const MAX = 4000;
        if (output.length > MAX) output = output.slice(0, MAX) + '\n... (truncated)';
        return output;
      } catch {
        // Fallback: command capture not available yet
        if (ctx.onCommandBlocked) {
          ctx.onCommandBlocked(command);
          return `Command "${command}" requires approval.`;
        }
        return `Command execution not available. Command: ${command}`;
      }
    }

    case 'search_files': {
      const query = (args.query || '').toLowerCase();
      const files = await invoke<string[]>('list_all_files', { path: ctx.projectRoot });
      const matches = files
        .filter(f => f.toLowerCase().includes(query))
        .slice(0, 20);
      return matches.length > 0 ? matches.join('\n') : 'No files found.';
    }

    case 'grep': {
      const pattern = args.pattern;
      if (!pattern) throw new Error('pattern is required');
      const searchPath = args.path
        ? resolvePath(args.path, ctx.projectRoot)
        : ctx.projectRoot;
      // Use list_all_files + read to simulate grep (until a dedicated backend command exists)
      const files = await invoke<string[]>('list_all_files', { path: searchPath });
      const results: string[] = [];
      const MAX_FILES = 20;
      const MAX_RESULTS = 30;
      for (const file of files.slice(0, MAX_FILES)) {
        if (results.length >= MAX_RESULTS) break;
        try {
          const fullPath = file.startsWith('/') ? file : `${ctx.projectRoot}/${file}`;
          const content = await invoke<string>('read_file_content', { path: fullPath });
          const lines = content.split('\n');
          for (let i = 0; i < lines.length && results.length < MAX_RESULTS; i++) {
            if (lines[i].includes(pattern)) {
              const rel = file.startsWith(ctx.projectRoot) ? file.slice(ctx.projectRoot.length + 1) : file;
              results.push(`${rel}:${i + 1}: ${lines[i].trim()}`);
            }
          }
        } catch { /* skip unreadable files */ }
      }
      return results.length > 0 ? results.join('\n') : `No matches for "${pattern}".`;
    }

    case 'list_dir': {
      const dirPath = args.path
        ? resolvePath(args.path, ctx.projectRoot)
        : ctx.projectRoot;
      const entries = await invoke<{ name: string; is_dir: boolean }[]>('read_dir_tree', { path: dirPath, depth: 1 });
      return entries
        .map(e => `${e.is_dir ? '📁' : '📄'} ${e.name}`)
        .slice(0, 50)
        .join('\n') || '(empty directory)';
    }

    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

// ── Helpers ──

function resolvePath(relativePath: string, projectRoot: string): string {
  if (!relativePath) throw new Error('path is required');
  // Resolve to absolute
  const resolved = relativePath.startsWith('/') ? relativePath : `${projectRoot}/${relativePath}`;
  // Normalize: collapse /./, resolve /../ segments
  const parts: string[] = [];
  for (const seg of resolved.split('/')) {
    if (seg === '.' || seg === '') continue;
    if (seg === '..') { parts.pop(); continue; }
    parts.push(seg);
  }
  const canonical = '/' + parts.join('/');
  // Verify the canonical path is within the project root (exact match or subpath)
  if (canonical !== projectRoot && !canonical.startsWith(projectRoot + '/')) {
    throw new Error('Path traversal not allowed: resolved path is outside project root');
  }
  return canonical;
}
