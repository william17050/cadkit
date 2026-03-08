# CadKit Development Roadmap

## Phase 1: Foundation (Q1 2026) — **Done**

### Milestone 1.1: Viewport Rendering
- [x] Integrate wgpu for 2D rendering
- [x] Draw entities in viewport
- [x] Pan and zoom with mouse
- [x] Grid display (dot grid)
- [x] Coordinate readout / status bar

### Milestone 1.2: Interactive Drawing
- [x] Line tool (command + toolbar)
- [x] Circle tool (center, radius/diameter)
- [x] Arc tool (3-point)
- [x] Polyline tool with close option
- [x] ESC/right-click cancels tools; command aliases supported

### Milestone 1.3: Snaps
- [x] Endpoint snap
- [x] Midpoint snap
- [x] Center snap
- [x] Intersection snap search with pixel radius
- [x] FROM offset workflow (base point + typed offset)
- [x] Nearest / perpendicular / tangent snaps
- [x] Visual snap glyphs (icon per snap type — square/triangle/circle/diamond/X/perp/tangent)

### Milestone 1.4: Selection & Basic Edits
- [x] Click and box selection (window/crossing)
- [x] Multi-select toggle (Shift)
- [x] Delete selected
- [x] Move / copy / rotate with rubber-band previews
- [x] Offset command (distance + side pick)
- [x] Trim command (cutting edges then trim)
- [x] Extend command (boundary pick then extend)
- [x] Undo / redo stack
- [x] Layer-aware selection highlight

---

## Phase 2: Annotations & Precision (Q2 2026) — **Done**

### Milestone 2.1: Linear Dimensions
- [x] `DimLinear` entity kind in 2d-core
- [x] 3-click DIMLINEAR placement workflow (`dli` / `dimlinear` / `dim`)
- [x] Live rubber-band preview with stroke-font distance label
- [x] Text direction normalized — readable regardless of pick order
- [x] Move / copy / rotate support for dim entities
- [x] Selection highlight and pick distance to dim line
- [x] **DimStyle dialog** — adjust text height, arrow size, color, precision
- [x] **Dim text via egui native font** — egui overlay painter renders distance label
- [x] Selected-dimension grip editing (start/end/offset/text)
- [x] Arrow/text overflow improvements (outside arrows when short span, text moved outside with leader)
- [x] FROM-in-dimension second point (`FR` distance entry) reliability polish
- [x] Angular dimension (`DIMANGULAR`)
- [x] Radial / diameter dimension (`DIMRADIUS` / `DIMDIAMETER`)
- [x] DXF DIMENSION entity export
- [x] DXF DIMENSION entity import

### Milestone 2.2: Text Placement
- [x] `Text` entity kind in 2d-core (content, insertion point, height, rotation)
- [x] `TEXT` command — pick insertion point, enter height/rotation, type content (`T`)
- [x] Text rendered via egui overlay (not stroke font)
- [x] Text selection, move, rotate, properties editing (`ET` to edit content)
- [x] DXF TEXT / MTEXT export and import
- [x] Font name stored in drawing (`font_name` persisted on Text entity; DXF style name round-tripped)

### Milestone 2.3: Advanced Editing
- [x] Scale command
- [x] Mirror command
- [x] Fillet command (radius + two picks: line/polyline segment support)
- [x] Fillet rebuild behavior for polyline workflows
  - [x] Same open polyline corner fillet rebuilds one continuous polyline
  - [x] Same closed polyline corner fillet rebuilds closed polyline with sampled arc
  - [x] Mixed polyline + line endpoint fillet can rebuild joined polyline result
- [x] PEDIT command (`PE`/`PEDIT`) — select open polyline, then join touching line/arc at ends
- [x] JOIN alias (`J`/`JOIN`) for selected touching open polyline + segments
- [x] Chamfer command (single or dual distance, including 0 for sharp corner)
- [x] Polygon command (`POL`/`POLYGON`) with side count + center/radius rubber-band (ortho-aware)
- [x] Ellipse command (`EL`/`ELLIPSE`) with center/radius/height rubber-band (ortho-aware)
- [x] Rectangle command (`REC`/`RECTANGLE`) with diagonal or dimensions (`w,h`) workflows + rubber-band
- [x] Array command (rectangular / polar)
  - [x] Rectangular grip workflow with live ghost preview and on-screen grip handles (`dx`, `dy`, `cols`, `rows`)
  - [x] Typed value entry while grip is active (exact spacing/count), with grip auto-release
  - [x] Count grip supports minimum of 1 row/column
  - [x] Associative rectangular arrays (re-editable later by selecting array members and running `ARRAY`)
  - [x] Group-style array selection behavior (member pick selects full array group)
  - [x] `E` alias in array grip edit mode explodes associative linkage and exits

### Milestone 2.4: Layers & Organization
- [x] Create / delete / rename layers
- [x] Layer color edit; set current layer
- [x] Move entities between layers (combo box + Assign in Properties panel)
- [x] Layer visibility toggle (eye icon; filters rendering via `visible_entities()`)
- [x] Layer locking enforcement for edit commands and property edits
- [x] Layer freeze (dedicated freeze toggle; frozen layers are hidden and non-editable)

### Milestone 2.5: Precision & UI Polish
- [x] Relative coordinates (@x,y) and polar (@dist<angle)
- [x] Direct distance entry with live rubber-band
- [x] Ortho mode (F8) and snap toggle (F3)
- [x] Status bar — live cursor world coordinates, active layer, snap/ortho state
- [x] Perpendicular / parallel tracking snaps
- [x] Preference persistence (last file, grid spacing, snap/ortho/grid, dim style)
- [x] Recent files list in File menu
- [x] Basic linetypes (Continuous / Hidden / Center) for line-based entities
- [x] Global linetype scale command (`LTS` / `LTSCALE`)
- [x] Layer linetype + layer LT scale style controls
- [x] Per-entity `ByLayer` linetype with per-entity override
- [x] Per-entity `ByLayer` LT scale with numeric override

---

## Phase 3: File Interop & IO (Q3 2026)

### Milestone 3.1: DXF Completeness
- [x] DIMENSION entity export (RotatedDimension / AlignedDimension)
- [x] TEXT / MTEXT entity export and import
- [x] HATCH entity stub (ASCII DXF fallback: import boundary points as closed polylines)
- [x] Block (INSERT) import as flattened geometry
- [x] Arc direction handling edge cases (DXF import normalizes arcs to stored CCW convention)

### Milestone 3.2: Additional Formats
- [x] SVG export (paths only)
- [x] PDF export (single-page vector, auto-fit visible geometry)
- [x] Auto-save / recovery file (20s recovery snapshot + startup restore/discard prompt)

---

## Phase 4: Python & AI (Q4 2026)

### Milestone 4.1: Python Bridge
- [x] PyO3 integration working (new `cadkit-scripting-python` crate in workspace)
- [x] `cad.line()`, `cad.circle()`, `cad.arc()` API (embedded `cad` object in Python engine)
- [x] `cad.get_entity()` / `cad.select()` queries
- [x] `cad.dim_linear()` API
- [x] Python console in UI (`PYCON` alias + File menu window)
- [x] Run .py scripts from file (`PYRUN`/`PY` alias + File menu picker; `./run.sh --py` launcher)

### Milestone 4.2: AI Command Line
- [x] Detect Claude Desktop via MCP (local config/process detection + status in `AICMD`)
- [x] Parse natural language commands (initial local parser: line/circle/arc/dimlinear)
- [x] Generate Python code from intent (preview in `AICMD` window)
- [x] Execute generated code with preview (explicit "Execute Preview" action)
- [x] Fallback to local model backend (LM Studio OpenAI-compatible endpoint, with local parser fallback)

### Milestone 4.3: AI Code Completion
- [x] Phi-3 model integration (initial local `llama-cli` GGUF runtime path in `AICMD`)
- [x] Autocomplete in Python console (`Ctrl+Space` + `Complete` + API hint inserts)
- [x] API documentation lookup (in-app `AIHELP` CadKit Python cheat-sheet for `AICMD`)
- [x] Example code suggestions (API examples injected via `AIHELP` into generation context)

---

## Phase 5: Advanced 2D (Q1 2027)

### Milestone 5.1: Hatch & Regions
- [x] Boundary detection algorithm (first pass in geometry crate + `BOUNDARY` point-pick command)
- [ ] Gap healing under tolerance
- [x] Island detection (holes) with dialog toggle (`Detect Islands` on/off)
- [x] First-pass `HATCH` command (pick point + line hatch generation with spacing/angle/LTScale/color)
- [x] Hatch patterns (initial built-ins: ANSI31 / ANSI32 / ANSI37 / Cross / Grid)
- [x] Use region detection for hatch fill (line + polyline + circle/arc boundaries)
- [x] Hatch dialog polish (pattern dropdown, sample tiles, ACI color picker 1-255, layer color inherit/override)

### Milestone 5.2: Blocks & References
- [x] Block definition
- [x] Insert block reference (true `Insert` entity, non-exploded by default)
- [x] Block editor (first pass: `BEDIT` / `BSAVE` / `BCANCEL`)
- [ ] Nested blocks
- [~] Block library / palette (name list + insert flow; thumbnails pending)

### Milestone 5.3: Leaders & Callouts
- [ ] Leader entity (line + arrowhead + text)
- [ ] Multileader
- [ ] Balloon callouts

---

## Phase 6: Direct 3D (Q2-Q3 2027)

### Milestone 6.1: 3D Viewport
- [ ] 3D camera controls (orbit / pan / zoom)
- [ ] Perspective / orthographic toggle
- [ ] View cube navigation
- [ ] Shaded rendering mode

### Milestone 6.2: Push/Pull
- [ ] Select 2D region (face)
- [ ] Extrude in Z direction with live preview
- [ ] Confirm to create solid mesh
- [ ] Extrude with taper angle

### Milestone 6.3: 3D Boolean Operations
- [ ] Union solids
- [ ] Subtract solids
- [ ] Intersect solids
- [ ] Manifold mesh validation

---

## Phase 7: CNC/CAM (Q4 2027 - Q1 2028)

### Milestone 7.1: Toolpath Generation
- [ ] 2D profile toolpath
- [ ] Pocket clearing
- [ ] Drilling operations
- [ ] Adaptive clearing
- [ ] Helical entry

### Milestone 7.2: Post-Processors
- [ ] Generic G-code output
- [ ] Mach3 post-processor
- [ ] Fanuc/KOMO post-processor
- [ ] Planet CNC post-processor
- [ ] Load Aspire .pp files

### Milestone 7.3: CAM Features
- [ ] Tool library
- [ ] Material database
- [ ] Feed/speed calculator
- [ ] Toolpath simulation
- [ ] Collision detection
- [ ] Test on home CNC

---

## Phase 8: Cabinet Designer (Q2-Q3 2028)

### Milestone 8.1: Cabinet Primitives
- [ ] Base cabinet template
- [ ] Wall cabinet template
- [ ] Tall cabinet template
- [ ] Corner cabinet variants
- [ ] Drawer stack generator

### Milestone 8.2: Cut List & Nesting
- [ ] Generate cut list from design
- [ ] Sheet nesting algorithm
- [ ] Grain direction control
- [ ] Edge banding list
- [ ] Hardware requirements

### Milestone 8.3: Production Output
- [ ] CNC programs for nested sheets
- [ ] Assembly drawings
- [ ] Hardware location drilling
- [ ] Material cost estimation
- [ ] Quote generation

---

## Phase 9: Parametric & Polish (Q4 2028 - Q1 2029)

### Milestone 9.1: Constraints
- [ ] Distance constraints
- [ ] Angle constraints
- [ ] Parallel / perpendicular
- [ ] Tangent constraints
- [ ] Constraint solver

### Milestone 9.2: Feature History
- [ ] History tree UI
- [ ] Edit feature parameters
- [ ] Suppress / resume features
- [ ] Reorder operations
- [ ] Feature patterns

### Milestone 9.3: Final Polish
- [ ] Performance optimization
- [ ] User documentation
- [ ] Tutorial system
- [ ] Example projects
- [ ] Marketing materials

---

## Exit Strategy (2029)

- Market to cabinet shops
- Build customer testimonials
- Reach 500–2000 users
- $500K–2M ARR target
- Field acquisition offers
- Close deal or continue building

---

**Note**: Roadmap is aspirational. Actual timeline depends on:
- Full-time job constraints
- AI assistance effectiveness
- Feature complexity discoveries
- Market feedback

Priorities may shift based on user needs and technical constraints.
