# CadKit Development Roadmap

## Phase 1: Foundation (Current - Q1 2026)

### Milestone 1.1: Viewport Rendering ✓ NEXT
- [ ] Integrate wgpu for 2D rendering
- [ ] Draw lines in viewport from entity data
- [ ] Pan and zoom with mouse
- [ ] Grid display with adjustable spacing
- [ ] Coordinate display cursor tracking

### Milestone 1.2: Interactive Drawing
- [ ] Line tool: click-click to create line
- [ ] Circle tool: center-point, radius
- [ ] Arc tool: three-point arc
- [ ] Polyline tool: connected segments
- [ ] ESC to cancel current tool

### Milestone 1.3: Snaps
- [ ] Endpoint snap
- [ ] Midpoint snap
- [ ] Center snap (circles/arcs)
- [ ] Intersection snap
- [ ] Nearest point snap
- [ ] Snap indicator visual feedback

### Milestone 1.4: Selection & Basic Edits
- [ ] Click to select entity
- [ ] Box selection (window/crossing)
- [ ] Multiple selection with Shift
- [ ] Delete selected entities
- [ ] Move entities with mouse
- [ ] Properties panel shows selected entity info

## Phase 2: Core 2D Features (Q2-Q3 2026)

### Milestone 2.1: Advanced Editing
- [ ] Copy command
- [ ] Rotate command
- [ ] Scale command
- [ ] Mirror command
- [ ] Offset command
- [ ] Trim command
- [ ] Extend command
- [ ] Fillet command
- [ ] Chamfer command

### Milestone 2.2: Layers & Organization
- [ ] Create/delete layers
- [ ] Layer properties (color, linetype)
- [ ] Set current layer
- [ ] Move entities between layers
- [ ] Layer visibility toggle
- [ ] Layer locking
- [ ] Freeze/thaw layers

### Milestone 2.3: Precision Input
- [ ] Command-line coordinate input
- [ ] Distance/angle constraint input
- [ ] Relative coordinates (@x,y)
- [ ] Polar coordinates (distance<angle)
- [ ] Object tracking (ortho mode)

### Milestone 2.4: File Operations
- [ ] DXF import
- [ ] DXF export
- [ ] File browser dialog
- [ ] Recent files list
- [ ] Auto-save functionality

## Phase 3: Python & AI (Q4 2026)

### Milestone 3.1: Python Bridge
- [ ] PyO3 integration working
- [ ] cad.line() API exposed
- [ ] cad.circle() API exposed
- [ ] cad.get_entity() queries
- [ ] Python console in UI
- [ ] Run .py scripts from file

### Milestone 3.2: AI Command Line
- [ ] Detect Claude Desktop via MCP
- [ ] Parse natural language commands
- [ ] Generate Python code from intent
- [ ] Execute generated code
- [ ] Show preview before execution
- [ ] Fallback to local small model

### Milestone 3.3: AI Code Completion
- [ ] Phi-3 model integration
- [ ] Autocomplete in Python console
- [ ] API documentation lookup
- [ ] Example code suggestions

## Phase 4: Advanced 2D (Q1 2027)

### Milestone 4.1: Annotations
- [ ] Text entities
- [ ] Multi-line text
- [ ] Dimensions (linear, angular, radial)
- [ ] Leaders and callouts
- [ ] Hatch patterns

### Milestone 4.2: Region Detection
- [ ] Boundary detection algorithm
- [ ] Gap healing under tolerance
- [ ] Island detection (holes)
- [ ] Winding order calculation
- [ ] Use for hatch fill

### Milestone 4.3: Blocks & References
- [ ] Block definition
- [ ] Insert block reference
- [ ] Block editor
- [ ] Nested blocks
- [ ] Block library

## Phase 5: Direct 3D (Q2-Q3 2027)

### Milestone 5.1: 3D Viewport
- [ ] 3D camera controls
- [ ] Perspective/orthographic toggle
- [ ] View cube navigation
- [ ] Shaded rendering mode

### Milestone 5.2: Push/Pull
- [ ] Select 2D region (face)
- [ ] Extrude in Z direction
- [ ] Preview mesh during drag
- [ ] Confirm to create solid
- [ ] Extrude with taper angle

### Milestone 5.3: 3D Boolean Operations
- [ ] Union solids
- [ ] Subtract solids
- [ ] Intersect solids
- [ ] Manifold mesh validation

## Phase 6: CNC/CAM (Q4 2027 - Q1 2028)

### Milestone 6.1: Toolpath Generation
- [ ] 2D profile toolpath
- [ ] Pocket clearing
- [ ] Drilling operations
- [ ] Adaptive clearing
- [ ] Helical entry

### Milestone 6.2: Post-Processors
- [ ] Generic G-code output
- [ ] Mach3 post-processor
- [ ] Fanuc/KOMO post-processor
- [ ] Planet CNC post-processor
- [ ] Load Aspire .pp files

### Milestone 6.3: CAM Features
- [ ] Tool library
- [ ] Material database
- [ ] Feed/speed calculator
- [ ] Toolpath simulation
- [ ] Collision detection
- [ ] Test on home CNC

## Phase 7: Cabinet Designer (Q2-Q3 2028)

### Milestone 7.1: Cabinet Primitives
- [ ] Base cabinet template
- [ ] Wall cabinet template
- [ ] Tall cabinet template
- [ ] Corner cabinet variants
- [ ] Drawer stack generator

### Milestone 7.2: Cut List & Nesting
- [ ] Generate cut list from design
- [ ] Sheet nesting algorithm
- [ ] Grain direction control
- [ ] Edge banding list
- [ ] Hardware requirements

### Milestone 7.3: Production Output
- [ ] CNC programs for nested sheets
- [ ] Assembly drawings
- [ ] Hardware location drilling
- [ ] Material cost estimation
- [ ] Quote generation

## Phase 8: Parametric & Polish (Q4 2028 - Q1 2029)

### Milestone 8.1: Constraints
- [ ] Distance constraints
- [ ] Angle constraints
- [ ] Parallel/perpendicular
- [ ] Tangent constraints
- [ ] Constraint solver

### Milestone 8.2: Feature History
- [ ] History tree UI
- [ ] Edit feature parameters
- [ ] Suppress/resume features
- [ ] Reorder operations
- [ ] Feature patterns

### Milestone 8.3: Final Polish
- [ ] Performance optimization
- [ ] User documentation
- [ ] Tutorial system
- [ ] Example projects
- [ ] Marketing materials

## Exit Strategy (2029)

- Market to cabinet shops
- Build customer testimonials
- Reach 500-2000 users
- $500K-2M ARR target
- Field acquisition offers
- Close deal or continue building

---

**Note**: Roadmap is aspirational. Actual timeline depends on:
- Full-time job constraints
- AI assistance effectiveness
- Feature complexity discoveries
- Market feedback

Priorities may shift based on user needs and technical constraints.
