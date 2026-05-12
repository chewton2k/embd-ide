/** Layout engine for file diagram nodes. */

export interface LayoutNode {
  id: string;
  type: 'target' | 'import' | 'dependent' | 'export' | 'endpoint' | 'schema' | 'external' | 'call' | 'database';
  label: string;
  sublabel?: string;
  details?: string[];
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LayoutEdge {
  id: string;
  from: string;
  to: string;
  label?: string;
  dashed?: boolean;
}

export interface LayoutResult {
  nodes: LayoutNode[];
  edges: LayoutEdge[];
}

const NODE_W = 200;
const NODE_H = 44;
const GAP_X = 260;
const GAP_Y = 65;

export function computeLayout(graph: {
  target: { path: string; name: string };
  imports: { path: string; name: string; symbols: string[] }[];
  dependents: { path: string; name: string; symbols: string[] }[];
  exports: { name: string; kind: string; signature: string; params: string[]; return_type: string }[];
  endpoints: { method: string; route: string; handler: string; params: string[]; middleware: string[] }[];
  schemas: { name: string; kind: string; fields: { name: string; field_type: string; optional: boolean; constraints: string[] }[]; source: string }[];
  external_deps: string[];
  calls: { caller: string; callee: string; is_async: boolean }[];
}): LayoutResult {
  const nodes: LayoutNode[] = [];
  const edges: LayoutEdge[] = [];

  const targetId = 'target';
  nodes.push({
    id: targetId,
    type: 'target',
    label: graph.target.name,
    sublabel: graph.target.path,
    x: 0,
    y: 0,
    width: NODE_W,
    height: NODE_H,
  });

  // Imports — left of target
  const importCount = graph.imports.length;
  const importStartY = -((importCount - 1) * GAP_Y) / 2;
  graph.imports.forEach((imp, i) => {
    const id = `import-${i}`;
    nodes.push({
      id,
      type: 'import',
      label: imp.name,
      sublabel: imp.path,
      details: imp.symbols.length > 0 ? [`{ ${imp.symbols.join(', ')} }`] : undefined,
      x: -GAP_X,
      y: importStartY + i * GAP_Y,
      width: NODE_W,
      height: NODE_H,
    });
    edges.push({ id: `e-${id}`, from: id, to: targetId, label: imp.symbols.length > 0 ? imp.symbols.slice(0, 3).join(', ') : undefined });
  });

  // Dependents — right of target
  const depCount = graph.dependents.length;
  const depStartY = -((depCount - 1) * GAP_Y) / 2;
  graph.dependents.forEach((dep, i) => {
    const id = `dep-${i}`;
    nodes.push({
      id,
      type: 'dependent',
      label: dep.name,
      sublabel: dep.path,
      details: dep.symbols.length > 0 ? [`imports: ${dep.symbols.join(', ')}`] : undefined,
      x: GAP_X,
      y: depStartY + i * GAP_Y,
      width: NODE_W,
      height: NODE_H,
    });
    edges.push({ id: `e-${id}`, from: targetId, to: id });
  });

  // Exports — below target (with signature details)
  const exportCount = graph.exports.length;
  const exportStartX = -((exportCount - 1) * (NODE_W * 0.75)) / 2;
  graph.exports.forEach((exp, i) => {
    const id = `export-${i}`;
    const details: string[] = [];
    if (exp.signature) details.push(exp.signature);
    if (exp.params.length > 0) details.push(`params: ${exp.params.join(', ')}`);
    if (exp.return_type) details.push(`→ ${exp.return_type}`);
    nodes.push({
      id,
      type: 'export',
      label: exp.name,
      sublabel: exp.kind,
      details: details.length > 0 ? details : undefined,
      x: exportStartX + i * (NODE_W * 0.75),
      y: GAP_Y * 1.8,
      width: NODE_W * 0.85,
      height: details.length > 0 ? NODE_H + details.length * 14 : NODE_H,
    });
    edges.push({ id: `e-${id}`, from: targetId, to: id });
  });

  // Endpoints — above target (with method, params, middleware)
  const epCount = graph.endpoints.length;
  const epStartX = -((epCount - 1) * (NODE_W * 0.8)) / 2;
  graph.endpoints.forEach((ep, i) => {
    const id = `endpoint-${i}`;
    const details: string[] = [];
    if (ep.params.length > 0) details.push(`params: ${ep.params.join(', ')}`);
    if (ep.middleware.length > 0) details.push(`middleware: ${ep.middleware.join(' → ')}`);
    if (ep.handler) details.push(`handler: ${ep.handler}`);
    nodes.push({
      id,
      type: 'endpoint',
      label: `${ep.method} ${ep.route}`,
      details: details.length > 0 ? details : undefined,
      x: epStartX + i * (NODE_W * 0.8),
      y: -GAP_Y * 1.8,
      width: NODE_W * 0.95,
      height: details.length > 0 ? NODE_H + details.length * 14 : NODE_H,
    });
    edges.push({ id: `e-${id}`, from: targetId, to: id });
  });

  // Schemas — above-right (with fields)
  const schemaCount = graph.schemas.length;
  const schemaStartY = -((schemaCount - 1) * (GAP_Y * 0.9)) / 2;
  graph.schemas.forEach((sch, i) => {
    const id = `schema-${i}`;
    const isDb = sch.source && sch.source !== '' && sch.source !== 'type';
    const details: string[] = sch.fields.slice(0, 8).map(f => {
      let line = `${f.name}${f.optional ? '?' : ''}: ${f.field_type}`;
      if (f.constraints && f.constraints.length > 0) {
        line += ` [${f.constraints.join(', ')}]`;
      }
      return line;
    });
    if (sch.fields.length > 8) details.push(`... +${sch.fields.length - 8} more`);
    const nodeH = NODE_H + Math.min(details.length, 9) * 14;
    nodes.push({
      id,
      type: isDb ? 'database' : 'schema',
      label: sch.name,
      sublabel: isDb ? `${sch.source} ${sch.kind}` : sch.kind,
      details: details.length > 0 ? details : undefined,
      x: GAP_X * 0.7,
      y: -GAP_Y * 1.8 + schemaStartY + i * (GAP_Y * 0.9 + details.length * 7),
      width: NODE_W * 0.9,
      height: nodeH,
    });
    edges.push({ id: `e-${id}`, from: targetId, to: id, dashed: !isDb });
  });

  // External deps — below-left (grouped)
  const extCount = graph.external_deps.length;
  const extStartY = -((extCount - 1) * (GAP_Y * 0.55)) / 2;
  graph.external_deps.forEach((dep, i) => {
    const id = `ext-${i}`;
    nodes.push({
      id,
      type: 'external',
      label: dep,
      x: -GAP_X * 0.85,
      y: GAP_Y * 1.8 + extStartY + i * (GAP_Y * 0.55),
      width: NODE_W * 0.65,
      height: NODE_H * 0.8,
    });
    edges.push({ id: `e-${id}`, from: id, to: targetId, dashed: true });
  });

  // Calls — sequence-diagram-style edges between export nodes
  const exportIds = new Map(graph.exports.map((exp, i) => [exp.name, `export-${i}`]));
  graph.calls.forEach((call, i) => {
    const fromId = exportIds.get(call.caller);
    const toId = exportIds.get(call.callee);
    if (fromId && toId && fromId !== toId) {
      edges.push({
        id: `call-${i}`,
        from: fromId,
        to: toId,
        label: call.is_async ? 'await' : undefined,
        dashed: call.is_async,
      });
    }
  });

  return { nodes, edges };
}
