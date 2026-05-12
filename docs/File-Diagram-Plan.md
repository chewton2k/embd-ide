# File Diagram — Implementation Plan

## Overview

A visual dependency graph for any file in the project. Shows imports, exports, API endpoints, schemas, and relationships as an interactive node-and-arrow diagram on a pannable/zoomable canvas.

## User Flow

```
+ button (top-right) → "File Diagram" option → File search modal → 
User searches/selects file → Enter/click → Diagram opens as a tab
```

## What It Shows

For a selected file (e.g. `account.js`), the diagram renders:

- **Center node**: The selected file
- **Import nodes**: Files/modules this file imports (with arrows pointing in)
- **Dependent nodes**: Files that import this file (with arrows pointing out)
- **Export nodes**: Functions, classes, variables exported
- **API endpoints**: Routes defined in this file (GET /api/users, etc.)
- **Schema nodes**: Types, interfaces, database models defined
- **External deps**: npm packages / crate imports (grouped)

### Node Types & Colors

| Node Type | Color | Shape |
|-----------|-------|-------|
| Current file | Blue | Rounded rect |
| Local import | Green | Rounded rect |
| Dependent (imports this) | Orange | Rounded rect |
| Export (function/class) | Purple | Pill |
| API endpoint | Red | Pill |
| Schema/Type | Teal | Diamond |
| External package | Grey | Rect (dashed border) |

### Arrow Types

- Solid arrow: import relationship
- Dashed arrow: type-only import
- Labeled arrows: export name on the connection

---

## Architecture

```
┌─────────────────────────────────────────┐
│           Frontend (Svelte 5)            │
├─────────────────────────────────────────┤
│ FileDiagram.svelte                      │
│  ├─ DiagramCanvas.svelte (SVG canvas)   │
│  ├─ DiagramNode.svelte (each node)      │
│  └─ DiagramEdge.svelte (arrows)         │
├─────────────────────────────────────────┤
│ File search modal (reuse existing)      │
├─────────────────────────────────────────┤
│           Tauri IPC                      │
├─────────────────────────────────────────┤
│           Backend (Rust)                 │
│  analyze_file_graph command             │
│  - Parse imports/exports via regex      │
│  - Find dependents (grep project)       │
│  - Detect endpoints & schemas           │
└─────────────────────────────────────────┘
```

---

## Implementation

### Phase 1: Backend — File Analysis

**New Tauri command: `analyze_file_graph`**

Input: `{ file_path: String, project_root: String }`

Output:
```rust
struct FileGraph {
    target: FileNode,
    imports: Vec<ImportNode>,
    dependents: Vec<DependentNode>,
    exports: Vec<ExportNode>,
    endpoints: Vec<EndpointNode>,
    schemas: Vec<SchemaNode>,
    external_deps: Vec<String>,
}

struct FileNode { path: String, name: String }
struct ImportNode { path: String, name: String, symbols: Vec<String> }
struct DependentNode { path: String, name: String, symbols: Vec<String> }
struct ExportNode { name: String, kind: String } // kind: function, class, const, type
struct EndpointNode { method: String, route: String, handler: String }
struct SchemaNode { name: String, kind: String } // kind: interface, type, model, struct
```

**Parsing strategy (regex-based, no full AST):**

- **JS/TS imports**: `import .* from ['"](.+)['"]`, `require\(['"](.+)['"]\)`
- **JS/TS exports**: `export (default )?(function|class|const|let|type|interface) (\w+)`
- **API endpoints**: `(app|router)\.(get|post|put|delete|patch)\(['"](.+)['"]`
- **Schemas/Types**: `(interface|type|model|schema) (\w+)`
- **Rust**: `use `, `pub fn`, `pub struct`, `pub enum`, `mod`
- **Python**: `from X import`, `import X`, `def`, `class`, `@app.route`

**Finding dependents**: grep project files for imports of the target file path.

**Files to create:**
- `src-tauri/src/modules/graph/mod.rs`
- `src-tauri/src/modules/graph/parser.rs`
- `src-tauri/src/modules/graph/types.rs`

---

### Phase 2: Frontend — Canvas & Layout

**SVG-based canvas** (no external library needed):

- Container div with `overflow: hidden`, captures mouse/wheel events
- SVG element inside with a `<g>` transform group for pan/zoom
- Pan: mousedown + drag on background translates the group
- Zoom: wheel event scales the group (clamp 0.3x–3x)
- Nodes: `<foreignObject>` wrapping styled divs (for text wrapping, hover effects)
- Edges: `<path>` elements with arrowhead markers, cubic bezier curves

**Auto-layout algorithm (force-directed, simple):**

1. Place target file at center (0, 0)
2. Imports fan out to the left
3. Dependents fan out to the right
4. Exports below the target
5. Endpoints and schemas above
6. External deps in a cluster bottom-left
7. Apply basic collision avoidance (push overlapping nodes apart)

Nodes are draggable — user can rearrange after auto-layout.

**Files to create:**
- `src/lib/components/diagram/FileDiagram.svelte` — main tab component
- `src/lib/components/diagram/DiagramCanvas.svelte` — SVG pan/zoom canvas
- `src/lib/components/diagram/DiagramNode.svelte` — individual node
- `src/lib/components/diagram/DiagramEdge.svelte` — arrow/path between nodes
- `src/lib/components/diagram/layout.ts` — auto-layout logic

---

### Phase 3: Integration — Tab + Menu + Search

**+ button menu entry:**

Add "File Diagram" option to the existing + dropdown in the tabs bar.

**File search modal:**

Reuse the existing file search (same as Cmd+P / open file). On selection, open the diagram as a tab.

**Tab integration:**

- Diagram opens as a special tab (like terminal/preview)
- Tab title: "◈ account.js" (diamond icon + filename)
- Path stored as `__diagram__:/path/to/file`
- Multiple diagrams can be open simultaneously

**Files to modify:**
- `src/lib/components/tabs/Tabs.svelte` — add diagram tab rendering
- `src/App.svelte` — render FileDiagram when diagram path is active
- `src/lib/modules/stores/index.ts` — add `isDiagramPath()` helper

---

## Interaction Details

### Node interactions
- **Click node**: Highlight node + connected edges
- **Double-click file node**: Open that file in editor
- **Hover**: Show tooltip with full path / signature
- **Drag node**: Reposition (other nodes stay put)

### Canvas interactions
- **Scroll wheel**: Zoom in/out
- **Click + drag background**: Pan
- **Cmd+0**: Reset zoom to fit all nodes
- **Escape**: Close any tooltip/selection

### Toolbar (top of diagram)
- Zoom in / Zoom out / Fit to view buttons
- "Depth" selector: 1 (direct only) / 2 (2 levels deep) / 3
- Toggle checkboxes: Imports ✓ | Dependents ✓ | Exports ✓ | Endpoints ✓ | Schemas ✓
- "Refresh" button to re-analyze

---

## Example Output

For `src/routes/account.js`:

```
                    ┌──────────────┐
                    │ GET /account │
                    │ PUT /account │
                    └──────┬───────┘
                           │
  ┌────────────┐    ┌──────┴───────┐    ┌──────────────┐
  │ db/user.js │───▶│  account.js  │◀───│ app.js       │
  └────────────┘    └──────┬───────┘    └──────────────┘
  ┌────────────┐           │            ┌──────────────┐
  │ auth.js    │───▶       │       ◀────│ routes/index │
  └────────────┘           │            └──────────────┘
                    ┌──────┴───────┐
                    │ getAccount() │
                    │ updateUser() │
                    └──────────────┘
```

---

## Task Checklist

1. [ ] Create `analyze_file_graph` Rust command with regex parsers
2. [ ] Add language-specific import/export detection (JS/TS, Rust, Python)
3. [ ] Implement dependent-file search (grep project for imports of target)
4. [ ] Create SVG canvas component with pan/zoom
5. [ ] Implement auto-layout algorithm
6. [ ] Create node components (styled by type)
7. [ ] Create edge components (bezier arrows with labels)
8. [ ] Add node dragging
9. [ ] Add "File Diagram" to + button menu
10. [ ] Wire up file search modal → diagram tab
11. [ ] Add diagram tab type to App.svelte routing
12. [ ] Add toolbar (zoom controls, depth, toggles)
13. [ ] Add double-click-to-open-file interaction
14. [ ] Test with JS/TS, Rust, and Python files

---

## Timeline

| Step | Work | Estimate |
|------|------|----------|
| Backend parser | Regex-based analysis for JS/TS/Rust/Python | 2-3 days |
| SVG canvas | Pan, zoom, node rendering, edges | 3-4 days |
| Auto-layout | Force-directed positioning | 1-2 days |
| Integration | Tab system, + menu, file search | 1-2 days |
| Polish | Interactions, tooltips, styling | 1-2 days |

**Total: ~1.5-2 weeks**
