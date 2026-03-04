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
- [ ] Nearest / perpendicular / tangent snaps
- [ ] Visual snap glyphs (icon per snap type)

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

## Phase 2: Annotations & Precision (Q2 2026) — **In Progress**

### Milestone 2.1: Linear Dimensions
- [x] `DimLinear` entity kind in 2d-core
- [x] 3-click DIMLINEAR placement workflow (`dli` / `dimlinear` / `dim`)
- [x] Live rubber-band preview with stroke-font distance label
- [x] Text direction normalized — readable regardless of pick order
- [x] Move / copy / rotate support for dim entities
- [x] Selection highlight and pick distance to dim line
- [ ] **DimStyle dialog** — adjust text height, arrow size, extension line gap, color, precision
- [x] **Dim text via egui native font** — egui overlay painter renders distance label
- [ ] Angular dimension (`DIMANGULAR`)
- [ ] Radial / diameter dimension (`DIMRADIUS` / `DIMDIAMETER`)
- [ ] DXF DIMENSION entity export (currently skipped with warning)
- [ ] DXF DIMENSION entity import

### Milestone 2.2: Text Placement
- [x] `Text` entity kind in 2d-core (content, insertion point, height, rotation)
- [x] `TEXT` command — pick insertion point, enter height/rotation, type content (`T`)
- [x] Text rendered via egui overlay (not stroke font)
- [x] Text selection, move, rotate, properties editing (`ET` to edit content)
- [ ] DXF TEXT / MTEXT export and import
- [ ] Font name stored in drawing (currently height only)

### Milestone 2.3: Advanced Editing
- [ ] Scale command
- [ ] Mirror command
- [ ] Fillet command (radius + two lines/arcs)
- [ ] Chamfer command (distance × distance)
- [ ] Array command (rectangular / polar)

### Milestone 2.4: Layers & Organization
- [x] Create / delete / rename layers
- [x] Layer color edit; set current layer
- [x] Move entities between layers (combo box + Assign in Properties panel)
- [x] Layer visibility toggle (eye icon; filters rendering via `visible_entities()`)
- [ ] Layer locking / freeze (UI exists, enforcement not wired)

### Milestone 2.5: Precision & UI Polish
- [x] Relative coordinates (@x,y) and polar (@dist<angle)
- [x] Direct distance entry with live rubber-band
- [x] Ortho mode (F8) and snap toggle (F3)
- [x] Status bar — live cursor world coordinates, active layer, snap/ortho state
- [ ] Perpendicular / parallel tracking snaps
- [ ] Preference persistence (last file, grid spacing, snap/ortho flags)
- [ ] Recent files list in File menu

---

## Phase 3: File Interop & IO (Q3 2026)

### Milestone 3.1: DXF Completeness
- [ ] DIMENSION entity export (RotatedDimension / AlignedDimension)
- [ ] TEXT / MTEXT entity export and import
- [ ] HATCH entity stub (import bounding geometry)
- [ ] Block (INSERT) import as flattened geometry
- [ ] Arc direction handling edge cases

### Milestone 3.2: Additional Formats
- [ ] SVG export (paths only)
- [ ] PDF export (print layout)
- [ ] Auto-save / recovery file

---

## Phase 4: Python & AI (Q4 2026)

### Milestone 4.1: Python Bridge
- [ ] PyO3 integration working
- [ ] `cad.line()`, `cad.circle()`, `cad.arc()` API
- [ ] `cad.get_entity()` / `cad.select()` queries
- [ ] `cad.dim_linear()` API
- [ ] Python console in UI
- [ ] Run .py scripts from file

### Milestone 4.2: AI Command Line
- [ ] Detect Claude Desktop via MCP
- [ ] Parse natural language commands
- [ ] Generate Python code from intent
- [ ] Execute generated code with preview
- [ ] Fallback to local small model (Phi-3)

### Milestone 4.3: AI Code Completion
- [ ] Phi-3 model integration
- [ ] Autocomplete in Python console
- [ ] API documentation lookup
- [ ] Example code suggestions

---

## Phase 5: Advanced 2D (Q1 2027)

### Milestone 5.1: Hatch & Regions
- [ ] Boundary detection algorithm (region-find crate)
- [ ] Gap healing under tolerance
- [ ] Island detection (holes)
- [ ] Hatch patterns (ANSI, ISO, custom)
- [ ] Use region detection for hatch fill

### Milestone 5.2: Blocks & References
- [ ] Block definition
- [ ] Insert block reference
- [ ] Block editor
- [ ] Nested blocks
- [ ] Block library / palette

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
