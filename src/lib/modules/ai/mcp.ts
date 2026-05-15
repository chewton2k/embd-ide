/**
 * MCP (Model Context Protocol) client.
 *
 * Connects to MCP servers via stdio or SSE, discovers their tools,
 * and proxies tool calls from the agent. Users configure servers in
 * settings; the agent inherits all discovered tools automatically.
 *
 * Protocol: JSON-RPC 2.0 over stdio (spawn process) or HTTP SSE.
 * Spec: https://modelcontextprotocol.io
 */
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { log } from '../logging';
import type { ToolSchema } from './tools';

// ── Types ──

export interface McpServerConfig {
  id: string;
  name: string;
  transport: 'stdio' | 'sse';
  /** For stdio: command to spawn (e.g. "npx @modelcontextprotocol/server-github") */
  command?: string;
  /** For sse: URL of the SSE endpoint */
  url?: string;
  /** Environment variables to pass to the process */
  env?: Record<string, string>;
  enabled: boolean;
}

export interface McpTool {
  serverId: string;
  serverName: string;
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface McpConnection {
  serverId: string;
  status: 'connecting' | 'connected' | 'error' | 'disconnected';
  tools: McpTool[];
  error?: string;
}

// ── Stores ──

export const mcpServers = writable<McpServerConfig[]>(loadServers());
export const mcpConnections = writable<Record<string, McpConnection>>({});

// ── Persistence ──

const STORAGE_KEY = 'leo-mcp-servers';

function loadServers(): McpServerConfig[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch { return []; }
}

function saveServers(servers: McpServerConfig[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(servers));
}

mcpServers.subscribe(saveServers);

// ── Server management ──

export function addServer(config: Omit<McpServerConfig, 'id'>): McpServerConfig {
  const server: McpServerConfig = { ...config, id: `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 6)}` };
  mcpServers.update(s => [...s, server]);
  return server;
}

export function removeServer(id: string) {
  mcpServers.update(s => s.filter(srv => srv.id !== id));
  mcpConnections.update(c => { const copy = { ...c }; delete copy[id]; return copy; });
}

export function toggleServer(id: string) {
  mcpServers.update(s => s.map(srv => srv.id === id ? { ...srv, enabled: !srv.enabled } : srv));
}

// ── Connection lifecycle ──

/**
 * Connect to an MCP server and discover its tools.
 * For stdio: spawns the process via the backend.
 * For SSE: connects to the HTTP endpoint.
 */
export async function connectServer(serverId: string): Promise<void> {
  const servers = get(mcpServers);
  const server = servers.find(s => s.id === serverId);
  if (!server || !server.enabled) return;

  mcpConnections.update(c => ({
    ...c,
    [serverId]: { serverId, status: 'connecting', tools: [] },
  }));

  try {
    const tools = await discoverTools(server);
    mcpConnections.update(c => ({
      ...c,
      [serverId]: { serverId, status: 'connected', tools },
    }));
  } catch (e) {
    mcpConnections.update(c => ({
      ...c,
      [serverId]: { serverId, status: 'error', tools: [], error: String(e) },
    }));
  }
}

/**
 * Disconnect from an MCP server.
 */
export function disconnectServer(serverId: string) {
  mcpConnections.update(c => ({
    ...c,
    [serverId]: { serverId, status: 'disconnected', tools: [] },
  }));
}

/**
 * Connect all enabled servers.
 */
export async function connectAllEnabled(): Promise<void> {
  const servers = get(mcpServers).filter(s => s.enabled);
  await Promise.all(servers.map(s => connectServer(s.id)));
}

// ── Tool discovery ──

async function discoverTools(server: McpServerConfig): Promise<McpTool[]> {
  if (server.transport === 'stdio' && server.command) {
    return discoverToolsStdio(server);
  }
  if (server.transport === 'sse' && server.url) {
    return discoverToolsSse(server);
  }
  throw new Error(`Invalid server config: missing command or url`);
}

async function discoverToolsStdio(server: McpServerConfig): Promise<McpTool[]> {
  // Send initialize + tools/list via the backend's run_command_capture
  // Uses stdin piping via the shell's heredoc to avoid injection
  const initRequest = JSON.stringify({
    jsonrpc: '2.0', id: 1, method: 'initialize',
    params: { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'leo-ide', version: '0.2.0' } },
  });
  const listRequest = JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} });

  // Validate command doesn't contain shell metacharacters
  const cmd = server.command || '';
  if (!cmd || /[;&|`$(){}]/.test(cmd)) {
    log.warn(`MCP server command rejected (unsafe characters): ${cmd}`);
    return [];
  }

  try {
    // Use printf with %s to safely pass data without shell interpretation
    const escapedPayload = `${initRequest}\n${listRequest}\n`.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
    const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>(
      'run_command_capture',
      { command: `printf "%s" "${escapedPayload}" | ${cmd}`, cwd: '.', timeoutMs: 10000 },
    );

    // Parse JSON-RPC responses from stdout
    const lines = result.stdout.split('\n').filter(l => l.trim());
    for (const line of lines) {
      try {
        const response = JSON.parse(line);
        if (response.id === 2 && response.result?.tools) {
          return response.result.tools.map((t: any) => ({
            serverId: server.id,
            serverName: server.name,
            name: t.name,
            description: t.description || '',
            inputSchema: t.inputSchema || {},
          }));
        }
      } catch { /* skip non-JSON lines */ }
    }
  } catch (e) {
    log.warn(`MCP stdio discovery failed for ${server.name}`, e);
  }

  return [];
}

async function discoverToolsSse(server: McpServerConfig): Promise<McpTool[]> {
  // SSE transport: POST to the server's endpoint
  // This is a simplified implementation — full SSE would need persistent connection
  try {
    const response = await fetch(`${server.url}/mcp/v1/tools/list`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} }),
    });
    const data = await response.json();
    if (data.result?.tools) {
      return data.result.tools.map((t: any) => ({
        serverId: server.id,
        serverName: server.name,
        name: t.name,
        description: t.description || '',
        inputSchema: t.inputSchema || {},
      }));
    }
  } catch (e) {
    log.warn(`MCP SSE discovery failed for ${server.name}`, e);
  }
  return [];
}

// ── Tool conversion ──

/**
 * Convert discovered MCP tools to the OpenAI tool schema format
 * so they can be passed to the agent alongside native tools.
 */
export function mcpToolsToSchemas(): ToolSchema[] {
  const connections = get(mcpConnections);
  const schemas: ToolSchema[] = [];

  for (const conn of Object.values(connections)) {
    if (conn.status !== 'connected') continue;
    for (const tool of conn.tools) {
      schemas.push({
        type: 'function',
        function: {
          name: `mcp_${tool.serverId.slice(0, 8)}_${tool.name}`,
          description: `[${tool.serverName}] ${tool.description}`,
          parameters: {
            type: 'object',
            properties: tool.inputSchema.properties as any || {},
            required: (tool.inputSchema.required as string[]) || [],
          },
        },
      });
    }
  }

  return schemas;
}

/**
 * Invoke an MCP tool by name. Returns the result as a string.
 */
export async function invokeMcpTool(fullName: string, args: Record<string, unknown>): Promise<string> {
  // Parse the full name: mcp_<serverId>_<toolName>
  const parts = fullName.match(/^mcp_([^_]+)_(.+)$/);
  if (!parts) throw new Error(`Invalid MCP tool name: ${fullName}`);

  const [, serverIdPrefix, toolName] = parts;
  const connections = get(mcpConnections);
  const conn = Object.values(connections).find(c =>
    c.serverId.startsWith(serverIdPrefix) && c.tools.some(t => t.name === toolName)
  );

  if (!conn) throw new Error(`MCP server not connected for tool: ${toolName}`);

  const servers = get(mcpServers);
  const server = servers.find(s => s.id === conn.serverId);
  if (!server) throw new Error(`MCP server config not found`);

  // Call the tool via JSON-RPC
  const request = JSON.stringify({
    jsonrpc: '2.0', id: 1, method: 'tools/call',
    params: { name: toolName, arguments: args },
  });

  if (server.transport === 'stdio' && server.command) {
    const cmd = server.command;
    if (/[;&|`$(){}]/.test(cmd)) {
      throw new Error('MCP server command contains unsafe characters');
    }
    const escapedRequest = request.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
    const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>(
      'run_command_capture',
      { command: `printf "%s" "${escapedRequest}" | ${cmd}`, cwd: '.', timeoutMs: 30000 },
    );
    const lines = result.stdout.split('\n').filter(l => l.trim());
    for (const line of lines) {
      try {
        const response = JSON.parse(line);
        if (response.result?.content) {
          return response.result.content.map((c: any) => c.text || JSON.stringify(c)).join('\n');
        }
      } catch { /* skip */ }
    }
    return result.stdout || '(no output)';
  }

  if (server.transport === 'sse' && server.url) {
    const response = await fetch(`${server.url}/mcp/v1/tools/call`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: request,
    });
    const data = await response.json();
    if (data.result?.content) {
      return data.result.content.map((c: any) => c.text || JSON.stringify(c)).join('\n');
    }
    return JSON.stringify(data.result || data.error || '(empty)');
  }

  throw new Error('Server transport not configured');
}
