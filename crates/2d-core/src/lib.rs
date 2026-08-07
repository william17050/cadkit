//! 2D CAD core - entities, layers, and drawing management
//!
//! This crate provides:
//! - 2D geometric entities (Line, Arc, Circle, Polyline)
//! - Layer management
//! - Drawing document structure
//! - Entity storage and queries

pub mod dxf_io;
pub use dxf_io::{aci_to_rgb, rgb_to_aci, DxfImportResult};

use cadkit_types::{CadError, Guid, Result, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AutoCAD-style default layer colour palette (index 0–7).
pub const LAYER_COLORS: &[[u8; 3]] = &[
    [255, 255, 255], // 0 white
    [255, 0, 0],     // 1 red
    [255, 255, 0],   // 2 yellow
    [0, 255, 0],     // 3 green
    [0, 255, 255],   // 4 cyan
    [0, 0, 255],     // 5 blue
    [255, 0, 255],   // 6 magenta
    [128, 128, 128], // 7 gray
];

fn default_layer_color() -> [u8; 3] {
    LAYER_COLORS[0]
}
fn default_layer_frozen() -> bool {
    false
}

fn default_dim_arrow_length() -> f64 {
    3.0
}
fn default_dim_arrow_half_width() -> f64 {
    0.75
}
fn default_text_font_name() -> String {
    "STANDARD".to_string()
}
fn default_linetype() -> Linetype {
    Linetype::Continuous
}
fn default_entity_linetype_by_layer() -> bool {
    false
}
fn default_layer_linetype() -> Linetype {
    Linetype::Continuous
}
fn default_linetype_scale() -> f64 {
    1.0
}
fn default_blocks() -> HashMap<String, BlockDefinition> {
    HashMap::new()
}
fn default_block_params() -> BlockParamValues {
    BlockParamValues::default()
}
fn default_insert_dynamic_param_overrides() -> HashMap<Guid, f64> {
    HashMap::new()
}
fn default_insert_cabinet_param_overrides() -> HashMap<String, String> {
    HashMap::new()
}
fn default_axis_mask_true() -> bool {
    true
}

// =============================================================================
// Entities
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Linetype {
    Continuous,
    Hidden,
    Center,
}

impl Linetype {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Continuous => "Continuous",
            Self::Hidden => "Hidden",
            Self::Center => "Center",
        }
    }

    pub fn to_dxf_name(self) -> &'static str {
        match self {
            Self::Continuous => "CONTINUOUS",
            Self::Hidden => "HIDDEN",
            Self::Center => "CENTER",
        }
    }

    pub fn from_dxf_name(name: &str) -> Self {
        let n = name.trim();
        if n.eq_ignore_ascii_case("HIDDEN") {
            Self::Hidden
        } else if n.eq_ignore_ascii_case("CENTER") {
            Self::Center
        } else {
            Self::Continuous
        }
    }
}

/// Core 2D entity types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    /// Straight line segment
    Line {
        start: Vec3, // z=0 for 2D phase
        end: Vec3,
    },

    /// Circular arc
    Arc {
        center: Vec3,
        radius: f64,
        start_angle: f64, // radians, 0=+X axis, CCW positive
        end_angle: f64,
    },

    /// Full circle
    Circle { center: Vec3, radius: f64 },

    /// Connected line/arc segments
    Polyline { vertices: Vec<Vec3>, closed: bool },

    /// Aligned dimension between two points (line parallel to measured entity)
    DimAligned {
        start: Vec3,                   // first extension line origin
        end: Vec3,                     // second extension line origin
        offset: f64,                   // signed perpendicular distance to dimension line
        text_override: Option<String>, // None = auto-format measured distance
        text_pos: Vec3,                // world-space centre of dimension text
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Horizontal or vertical (linear) dimension
    DimLinear {
        start: Vec3, // first extension line origin
        end: Vec3,   // second extension line origin
        offset: f64, // signed displacement from mid-Y (horiz) or mid-X (vert) to dim line
        text_override: Option<String>,
        text_pos: Vec3,
        horizontal: bool, // true = measures X distance; false = measures Y distance
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Angular dimension between two rays from a common vertex.
    /// The arc spans CCW from angle(line1_pt) to angle(line2_pt) relative to the vertex.
    DimAngular {
        vertex: Vec3,   // angle apex
        line1_pt: Vec3, // point on first ray from vertex
        line2_pt: Vec3, // point on second ray from vertex
        radius: f64,    // dimension arc radius (set during Placing)
        text_override: Option<String>,
        text_pos: Vec3, // world-space centre of dimension text
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Radius or diameter dimension on a circle or arc.
    /// `is_diameter = false` → "R…" label with one arrowhead;
    /// `is_diameter = true`  → "Ø…" label with chord line + two arrowheads.
    DimRadial {
        center: Vec3,    // circle/arc centre
        radius: f64,     // actual radius
        leader_pt: Vec3, // user's click point (leader endpoint + text anchor)
        is_diameter: bool,
        text_override: Option<String>,
        text_pos: Vec3,
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Free-standing text label
    Text {
        position: Vec3, // insertion point (baseline-left), z=0
        content: String,
        height: f64,   // glyph height in world units
        rotation: f64, // CCW angle in radians from +X axis
        #[serde(default = "default_text_font_name")]
        font_name: String,
    },

    /// Block reference instance (true INSERT-style entity).
    Insert {
        name: String,
        position: Vec3,
        #[serde(default)]
        rotation: f64,
        #[serde(default = "default_linetype_scale")]
        scale_x: f64,
        #[serde(default = "default_linetype_scale")]
        scale_y: f64,
    },
}

impl EntityKind {
    /// Check if entity lies entirely on XY plane (z=0)
    pub fn is_planar(&self) -> bool {
        match self {
            EntityKind::Line { start, end } => {
                start.z.abs() < f64::EPSILON && end.z.abs() < f64::EPSILON
            }
            EntityKind::Arc { center, .. } | EntityKind::Circle { center, .. } => {
                center.z.abs() < f64::EPSILON
            }
            EntityKind::Polyline { vertices, .. } => {
                vertices.iter().all(|v| v.z.abs() < f64::EPSILON)
            }
            EntityKind::DimAligned {
                start,
                end,
                text_pos,
                ..
            }
            | EntityKind::DimLinear {
                start,
                end,
                text_pos,
                ..
            } => {
                start.z.abs() < f64::EPSILON
                    && end.z.abs() < f64::EPSILON
                    && text_pos.z.abs() < f64::EPSILON
            }
            EntityKind::DimAngular {
                vertex, text_pos, ..
            } => vertex.z.abs() < f64::EPSILON && text_pos.z.abs() < f64::EPSILON,
            EntityKind::DimRadial {
                center, text_pos, ..
            } => center.z.abs() < f64::EPSILON && text_pos.z.abs() < f64::EPSILON,
            EntityKind::Text { position, .. } => position.z.abs() < f64::EPSILON,
            EntityKind::Insert { position, .. } => position.z.abs() < f64::EPSILON,
        }
    }
}

/// Complete entity with ID and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: Guid,
    pub kind: EntityKind,
    pub layer: u32,
    /// Per-entity colour override. `None` means "ByLayer" (inherit from the layer).
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Simple built-in linetype. Defaults to `Continuous` for compatibility.
    #[serde(default = "default_linetype")]
    pub linetype: Linetype,
    /// When true, linetype is inherited from the layer.
    #[serde(default = "default_entity_linetype_by_layer")]
    pub linetype_by_layer: bool,
    /// Per-entity linetype scale override. `None` means "ByLayer".
    #[serde(default)]
    pub linetype_scale: Option<f64>,
    /// First-pass dynamic block parameter overrides for INSERT entities.
    #[serde(default = "default_block_params")]
    pub block_params: BlockParamValues,
    /// V1 dynamic parameter overrides for INSERT entities, keyed by parameter id.
    /// Empty means "use block parameter defaults".
    #[serde(default = "default_insert_dynamic_param_overrides")]
    pub insert_dynamic_param_overrides: HashMap<Guid, f64>,
    /// V1 cabinet parameter overrides for INSERT entities, keyed by cabinet parameter name.
    /// Empty means "use cabinet definition defaults".
    #[serde(default = "default_insert_cabinet_param_overrides")]
    pub insert_cabinet_param_overrides: HashMap<String, String>,
}

impl Entity {
    pub fn new(kind: EntityKind, layer: u32) -> Self {
        Self {
            id: Guid::new(),
            kind,
            layer,
            color: None,
            linetype: Linetype::Continuous,
            linetype_by_layer: false,
            linetype_scale: None,
            block_params: BlockParamValues::default(),
            insert_dynamic_param_overrides: HashMap::new(),
            insert_cabinet_param_overrides: HashMap::new(),
        }
    }
}

/// Stored entity payload inside a block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockEntity {
    pub kind: EntityKind,
    pub layer: u32,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default = "default_linetype")]
    pub linetype: Linetype,
    #[serde(default = "default_entity_linetype_by_layer")]
    pub linetype_by_layer: bool,
    #[serde(default)]
    pub linetype_scale: Option<f64>,
}

/// Simple first-pass dynamic block metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDynamic {
    #[serde(default)]
    pub enable_width: bool,
    #[serde(default)]
    pub enable_height: bool,
    #[serde(default = "default_linetype_scale")]
    pub base_width: f64,
    #[serde(default = "default_linetype_scale")]
    pub base_height: f64,
}

/// Per-insert dynamic parameter values.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct BlockParamValues {
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
}

/// Parameter value type exposed by a cabinet definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CabinetParameterType {
    Number,
    Integer,
    Boolean,
    Text,
    Choice,
}

/// Declarative cabinet parameter metadata used by cabinet instances and formulas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CabinetParameterDefinition {
    pub id: Guid,
    pub name: String,
    pub label: String,
    pub param_type: CabinetParameterType,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub choice_options: Vec<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Which authored view geometry a cabinet definition supplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CabinetViewKind {
    Plan,
    FrontElevation,
    Section,
}

/// A cabinet definition can carry multiple authored drawing views.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CabinetViewDefinition {
    pub kind: CabinetViewKind,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub entity_ids: Vec<Guid>,
}

/// Canonical part orientation / exposed-face hint for MTO and nesting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CabinetPartFace {
    None,
    Left,
    Right,
    Top,
    Bottom,
    Front,
    Back,
    Inside,
    Outside,
}

/// Grain guidance for rectangular sheet-good parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CabinetGrainDirection {
    None,
    AlongLength,
    AlongWidth,
}

/// One spreadsheet-style recipe row used to derive a cabinet part.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CabinetPartRecipeRow {
    pub id: Guid,
    pub part_name: String,
    #[serde(default = "default_recipe_enabled")]
    pub enabled: bool,
    #[serde(default = "default_recipe_formula_one")]
    pub qty_formula: String,
    pub length_formula: String,
    pub width_formula: String,
    pub thickness_formula: String,
    #[serde(default)]
    pub core_material_formula: String,
    #[serde(default)]
    pub finish_formula: String,
    #[serde(default)]
    pub face: Option<CabinetPartFace>,
    #[serde(default)]
    pub grain: Option<CabinetGrainDirection>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// First-pass cabinet metadata attached to a block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CabinetDefinition {
    pub family_name: String,
    #[serde(default)]
    pub family_kind: Option<String>,
    #[serde(default)]
    pub geometry_authored: bool,
    #[serde(default)]
    pub parameters: Vec<CabinetParameterDefinition>,
    #[serde(default)]
    pub views: Vec<CabinetViewDefinition>,
    #[serde(default)]
    pub part_recipe: Vec<CabinetPartRecipeRow>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Runtime value type produced by cabinet parameter parsing and recipe formulas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CabinetFormulaValue {
    Number(f64),
    Boolean(bool),
    Text(String),
}

/// Evaluated part output derived from one cabinet insert instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CabinetGeneratedPart {
    pub insert_entity_id: Guid,
    pub block_name: String,
    pub family_name: String,
    #[serde(default)]
    pub family_kind: Option<String>,
    pub part_name: String,
    pub quantity: f64,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    #[serde(default)]
    pub core_material: String,
    #[serde(default)]
    pub finish: String,
    #[serde(default)]
    pub face: Option<CabinetPartFace>,
    #[serde(default)]
    pub grain: Option<CabinetGrainDirection>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn default_recipe_enabled() -> bool {
    true
}

fn default_recipe_formula_one() -> String {
    "1".to_string()
}

impl CabinetFormulaValue {
    fn as_number(&self, context: &str) -> Result<f64> {
        match self {
            Self::Number(v) => Ok(*v),
            Self::Boolean(v) => Ok(if *v { 1.0 } else { 0.0 }),
            Self::Text(_) => Err(CadError::InvalidOperation(format!(
                "{context} must evaluate to a number"
            ))),
        }
    }

    fn as_bool(&self, context: &str) -> Result<bool> {
        match self {
            Self::Boolean(v) => Ok(*v),
            Self::Number(v) => Ok(v.abs() > 1e-12),
            Self::Text(v) => {
                let normalized = v.trim();
                if normalized.eq_ignore_ascii_case("true") {
                    Ok(true)
                } else if normalized.eq_ignore_ascii_case("false") || normalized.is_empty() {
                    Ok(false)
                } else {
                    Err(CadError::InvalidOperation(format!(
                        "{context} must evaluate to a boolean"
                    )))
                }
            }
        }
    }

    fn as_text(&self, _context: &str) -> String {
        match self {
            Self::Text(v) => v.clone(),
            Self::Boolean(v) => {
                if *v {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Self::Number(v) => {
                if (v.round() - *v).abs() <= 1e-12 {
                    format!("{}", *v as i64)
                } else {
                    v.to_string()
                }
            }
        }
    }
}

fn parse_cabinet_parameter_value(
    param: &CabinetParameterDefinition,
    raw: &str,
) -> Result<CabinetFormulaValue> {
    match param.param_type {
        CabinetParameterType::Number | CabinetParameterType::Integer => raw
            .trim()
            .parse::<f64>()
            .map(CabinetFormulaValue::Number)
            .map_err(|_| {
                CadError::InvalidOperation(format!(
                    "Cabinet parameter '{}' expects a numeric value, got '{}'",
                    param.name, raw
                ))
            }),
        CabinetParameterType::Boolean => {
            let normalized = raw.trim();
            if normalized.eq_ignore_ascii_case("true")
                || normalized == "1"
                || normalized.eq_ignore_ascii_case("yes")
            {
                Ok(CabinetFormulaValue::Boolean(true))
            } else if normalized.eq_ignore_ascii_case("false")
                || normalized == "0"
                || normalized.eq_ignore_ascii_case("no")
                || normalized.is_empty()
            {
                Ok(CabinetFormulaValue::Boolean(false))
            } else {
                Err(CadError::InvalidOperation(format!(
                    "Cabinet parameter '{}' expects a boolean value, got '{}'",
                    param.name, raw
                )))
            }
        }
        CabinetParameterType::Text | CabinetParameterType::Choice => {
            Ok(CabinetFormulaValue::Text(raw.to_string()))
        }
    }
}

fn evaluate_cabinet_formula(
    formula: &str,
    context: &HashMap<String, CabinetFormulaValue>,
) -> Result<CabinetFormulaValue> {
    let formula = formula.trim();
    if formula.is_empty() {
        return Ok(CabinetFormulaValue::Text(String::new()));
    }
    let mut parser = CabinetFormulaParser::new(formula, context);
    let value = parser.parse_expression()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(CadError::InvalidOperation(format!(
            "Unexpected trailing input in formula '{}'",
            formula
        )));
    }
    Ok(value)
}

struct CabinetFormulaParser<'a> {
    input: &'a str,
    pos: usize,
    context: &'a HashMap<String, CabinetFormulaValue>,
}

impl<'a> CabinetFormulaParser<'a> {
    fn new(input: &'a str, context: &'a HashMap<String, CabinetFormulaValue>) -> Self {
        Self {
            input,
            pos: 0,
            context,
        }
    }

    fn parse_expression(&mut self) -> Result<CabinetFormulaValue> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<CabinetFormulaValue> {
        let mut left = self.parse_additive()?;
        loop {
            self.skip_ws();
            let op = if self.consume_str("==") {
                Some("==")
            } else if self.consume_str("!=") {
                Some("!=")
            } else if self.consume_str(">=") {
                Some(">=")
            } else if self.consume_str("<=") {
                Some("<=")
            } else if self.consume_char('>') {
                Some(">")
            } else if self.consume_char('<') {
                Some("<")
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_additive()?;
            left = CabinetFormulaValue::Boolean(match op {
                "==" => compare_formula_values(&left, &right)? == 0,
                "!=" => compare_formula_values(&left, &right)? != 0,
                ">" => compare_formula_values(&left, &right)? > 0,
                "<" => compare_formula_values(&left, &right)? < 0,
                ">=" => compare_formula_values(&left, &right)? >= 0,
                "<=" => compare_formula_values(&left, &right)? <= 0,
                _ => unreachable!(),
            });
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<CabinetFormulaValue> {
        let mut left = self.parse_multiplicative()?;
        loop {
            self.skip_ws();
            if self.consume_char('+') {
                let right = self.parse_multiplicative()?;
                left = CabinetFormulaValue::Number(
                    left.as_number("left side of '+'")? + right.as_number("right side of '+'")?,
                );
            } else if self.consume_char('-') {
                let right = self.parse_multiplicative()?;
                left = CabinetFormulaValue::Number(
                    left.as_number("left side of '-'")? - right.as_number("right side of '-'")?,
                );
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<CabinetFormulaValue> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.consume_char('*') {
                let right = self.parse_unary()?;
                left = CabinetFormulaValue::Number(
                    left.as_number("left side of '*'")? * right.as_number("right side of '*'")?,
                );
            } else if self.consume_char('/') {
                let right = self.parse_unary()?;
                let denom = right.as_number("right side of '/'")?;
                if denom.abs() <= 1e-12 {
                    return Err(CadError::InvalidOperation(
                        "Division by zero in cabinet formula".to_string(),
                    ));
                }
                left = CabinetFormulaValue::Number(left.as_number("left side of '/'")? / denom);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<CabinetFormulaValue> {
        self.skip_ws();
        if self.consume_char('-') {
            return Ok(CabinetFormulaValue::Number(
                -self.parse_unary()?.as_number("unary '-' operand")?,
            ));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<CabinetFormulaValue> {
        self.skip_ws();
        if self.consume_char('(') {
            let value = self.parse_expression()?;
            self.skip_ws();
            if !self.consume_char(')') {
                return Err(CadError::InvalidOperation(
                    "Missing closing ')' in cabinet formula".to_string(),
                ));
            }
            return Ok(value);
        }

        if self.peek_char() == Some('"') {
            return Ok(CabinetFormulaValue::Text(self.parse_string_literal()?));
        }

        if let Some(number) = self.parse_number_literal()? {
            return Ok(CabinetFormulaValue::Number(number));
        }

        let ident = self.parse_identifier()?;
        self.skip_ws();
        if self.consume_char('(') {
            return self.parse_function_call(&ident);
        }

        if ident.eq_ignore_ascii_case("true") {
            return Ok(CabinetFormulaValue::Boolean(true));
        }
        if ident.eq_ignore_ascii_case("false") {
            return Ok(CabinetFormulaValue::Boolean(false));
        }

        self.context.get(&ident).cloned().ok_or_else(|| {
            CadError::InvalidOperation(format!("Unknown cabinet formula identifier '{}'", ident))
        })
    }

    fn parse_function_call(&mut self, name: &str) -> Result<CabinetFormulaValue> {
        let mut args = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_char(')') {
                break;
            }
            args.push(self.parse_expression()?);
            self.skip_ws();
            if self.consume_char(')') {
                break;
            }
            if !self.consume_char(',') {
                return Err(CadError::InvalidOperation(format!(
                    "Expected ',' in function '{}'",
                    name
                )));
            }
        }

        if name.eq_ignore_ascii_case("if") {
            if args.len() != 3 {
                return Err(CadError::InvalidOperation(
                    "Function 'if' expects exactly 3 arguments".to_string(),
                ));
            }
            return if args[0].as_bool("if condition")? {
                Ok(args[1].clone())
            } else {
                Ok(args[2].clone())
            };
        }
        if name.eq_ignore_ascii_case("min") {
            if args.len() != 2 {
                return Err(CadError::InvalidOperation(
                    "Function 'min' expects exactly 2 arguments".to_string(),
                ));
            }
            return Ok(CabinetFormulaValue::Number(
                args[0]
                    .as_number("min arg 1")?
                    .min(args[1].as_number("min arg 2")?),
            ));
        }
        if name.eq_ignore_ascii_case("max") {
            if args.len() != 2 {
                return Err(CadError::InvalidOperation(
                    "Function 'max' expects exactly 2 arguments".to_string(),
                ));
            }
            return Ok(CabinetFormulaValue::Number(
                args[0]
                    .as_number("max arg 1")?
                    .max(args[1].as_number("max arg 2")?),
            ));
        }

        Err(CadError::InvalidOperation(format!(
            "Unsupported cabinet formula function '{}'",
            name
        )))
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        if !self.consume_char('"') {
            return Err(CadError::InvalidOperation(
                "Expected string literal".to_string(),
            ));
        }
        let mut out = String::new();
        while let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let Some(escaped) = self.peek_char() else {
                        return Err(CadError::InvalidOperation(
                            "Unterminated escape sequence in string literal".to_string(),
                        ));
                    };
                    self.pos += escaped.len_utf8();
                    out.push(match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                }
                other => out.push(other),
            }
        }
        Err(CadError::InvalidOperation(
            "Unterminated string literal in cabinet formula".to_string(),
        ))
    }

    fn parse_number_literal(&mut self) -> Result<Option<f64>> {
        self.skip_ws();
        let rest = &self.input[self.pos..];
        let mut chars = rest.char_indices().peekable();
        let mut end = 0usize;
        let mut saw_digit = false;
        let mut saw_dot = false;

        while let Some((idx, ch)) = chars.peek().copied() {
            if ch.is_ascii_digit() {
                saw_digit = true;
                end = idx + ch.len_utf8();
                chars.next();
            } else if ch == '.' && !saw_dot {
                saw_dot = true;
                end = idx + ch.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        if !saw_digit {
            return Ok(None);
        }

        let token = &rest[..end];
        let value = token.parse::<f64>().map_err(|_| {
            CadError::InvalidOperation(format!(
                "Invalid numeric literal '{}' in cabinet formula",
                token
            ))
        })?;
        self.pos += end;
        Ok(Some(value))
    }

    fn parse_identifier(&mut self) -> Result<String> {
        self.skip_ws();
        let rest = &self.input[self.pos..];
        let mut chars = rest.char_indices();
        let Some((_, first)) = chars.next() else {
            return Err(CadError::InvalidOperation(
                "Unexpected end of cabinet formula".to_string(),
            ));
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(CadError::InvalidOperation(format!(
                "Expected identifier in cabinet formula near '{}'",
                rest
            )));
        }
        let mut end = first.len_utf8();
        for (idx, ch) in chars {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                end = idx + ch.len_utf8();
            } else {
                break;
            }
        }
        let ident = &rest[..end];
        self.pos += end;
        Ok(ident.to_string())
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, expected: &str) -> bool {
        if self.input[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            true
        } else {
            false
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn compare_formula_values(left: &CabinetFormulaValue, right: &CabinetFormulaValue) -> Result<i32> {
    match (left, right) {
        (CabinetFormulaValue::Number(a), CabinetFormulaValue::Number(b)) => {
            Ok(if (a - b).abs() <= 1e-12 {
                0
            } else if a < b {
                -1
            } else {
                1
            })
        }
        (CabinetFormulaValue::Boolean(a), CabinetFormulaValue::Boolean(b)) => Ok(match (*a, *b) {
            (false, false) | (true, true) => 0,
            (false, true) => -1,
            (true, false) => 1,
        }),
        _ => {
            let left_text = left.as_text("comparison");
            let right_text = right.as_text("comparison");
            Ok(if left_text == right_text {
                0
            } else if left_text < right_text {
                -1
            } else {
                1
            })
        }
    }
}

/// Authored block-local entity payload used as the regeneration source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockAuthoredEntity {
    pub local_entity_id: Guid,
    pub kind: EntityKind,
    pub layer: u32,
}

/// Axis for a dynamic parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterAxis {
    X,
    Y,
}

/// User-defined parameter metadata exposed by the block editor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub id: Guid,
    pub name: String,
    pub axis: ParameterAxis,
    pub default_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub step: f64,
}

/// Editor category for an action binding (runtime behavior is per-target).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Move,
    Stretch,
    Anchor,
    Visibility,
}

/// Runtime behavior for one bound action target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityBehavior {
    MoveRigid,
    KeepCentered,
    AnchorToCenter,
    AnchorToEdge,
    StretchFromLeft,
    StretchFromRight,
    StretchFromCenter,
    Ignore,
}

/// Block-local frame used to resolve offsets and placement rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceFrame {
    BlockOrigin,
    BoundsCenter,
    LeftEdge,
    RightEdge,
    TopEdge,
    BottomEdge,
}

/// How a target should be positioned relative to a reference frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlacementRule {
    KeepDefault,
    Offset(f64),
    Proportional(f64),
}

/// Axis participation mask for target updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxisMask {
    #[serde(default = "default_axis_mask_true")]
    pub x: bool,
    #[serde(default = "default_axis_mask_true")]
    pub y: bool,
}

impl Default for AxisMask {
    fn default() -> Self {
        Self { x: true, y: true }
    }
}

/// Selection target for an action: full entity, rigid group, or sub-entity handle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetRef {
    Entity(Guid),
    Group(Guid),
    SubEntity { entity_id: Guid, handle: u32 },
}

/// One target entry bound to an action with target-specific behavior and references.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionTarget {
    pub target: TargetRef,
    pub behavior: EntityBehavior,
    pub reference_frame: ReferenceFrame,
    pub placement_rule: PlacementRule,
    #[serde(default)]
    pub axis_mask: AxisMask,
    #[serde(default = "default_linetype_scale")]
    pub weight: f64,
}

/// Parameter-driven action binding that owns an editor category and target set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionBinding {
    pub id: Guid,
    pub parameter_id: Guid,
    pub action_kind: ActionKind,
    #[serde(default)]
    pub targets: Vec<ActionTarget>,
    #[serde(default)]
    pub order: i32,
}

/// Authoring-time rigid group definition for move-style targets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidGroupDefinition {
    pub id: Guid,
    pub name: String,
    #[serde(default)]
    pub members: Vec<Guid>,
}

/// Bounds of the authored block geometry in local coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockBounds {
    pub min: Vec2,
    pub max: Vec2,
}

/// Full dynamic metadata for a block definition (authoring model).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DynamicBlockDefinition {
    pub block_name: String,
    #[serde(default)]
    pub base_entities: Vec<BlockAuthoredEntity>,
    pub base_bounds: BlockBounds,
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,
    #[serde(default)]
    pub actions: Vec<ActionBinding>,
    #[serde(default)]
    pub groups: Vec<RigidGroupDefinition>,
}

/// Per-instance parameter value storage for an INSERT.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BlockInstanceDynamicState {
    pub insert_entity_id: Guid,
    #[serde(default)]
    pub param_values: HashMap<Guid, f64>,
}

/// Named reusable block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDefinition {
    pub name: String,
    pub base_point: Vec3,
    pub entities: Vec<BlockEntity>,
    #[serde(default)]
    pub dynamic: Option<BlockDynamic>,
    /// V1 dynamic authoring/action model (Phase 1 data-only).
    #[serde(default)]
    pub dynamic_v1: Option<DynamicBlockDefinition>,
    /// V1 cabinet-definition metadata for library-driven cabinet workflows.
    #[serde(default)]
    pub cabinet_v1: Option<CabinetDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockFileV1 {
    format: String,
    version: u32,
    block: BlockDefinition,
}

// =============================================================================
// Layers
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    #[serde(default = "default_layer_frozen")]
    pub frozen: bool,
    /// RGB colour used when rendering entities on this layer.
    /// Defaults to white so that old `.cadkit` files without this field load cleanly.
    #[serde(default = "default_layer_color")]
    pub color: [u8; 3],
    /// Layer linetype for entities set to "ByLayer".
    #[serde(default = "default_layer_linetype")]
    pub linetype: Linetype,
    /// Layer linetype scale for entities set to "ByLayer" scale.
    #[serde(default = "default_linetype_scale")]
    pub linetype_scale: f64,
}

impl Layer {
    pub fn new(id: u32, name: String, color: [u8; 3]) -> Self {
        Self {
            id,
            name,
            visible: true,
            locked: false,
            frozen: false,
            color,
            linetype: Linetype::Continuous,
            linetype_scale: 1.0,
        }
    }
}

// =============================================================================
// Drawing Document
// =============================================================================

/// Main drawing document containing all entities and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drawing {
    pub id: Guid,
    pub name: String,
    #[serde(default = "default_linetype_scale")]
    pub linetype_scale: f64,
    entities: HashMap<Guid, Entity>,
    #[serde(default = "default_blocks")]
    blocks: HashMap<String, BlockDefinition>,
    layers: HashMap<u32, Layer>,
    next_layer_id: u32,
    // TODO: Add units, limits, view settings
}

impl Drawing {
    pub fn new(name: String) -> Self {
        let mut layers = HashMap::new();
        let default_layer = Layer::new(0, "0".to_string(), LAYER_COLORS[0]);
        layers.insert(0, default_layer);

        Self {
            id: Guid::new(),
            name,
            linetype_scale: 1.0,
            entities: HashMap::new(),
            blocks: HashMap::new(),
            layers,
            next_layer_id: 1,
        }
    }

    // -------------------------------------------------------------------------
    // Entity Management
    // -------------------------------------------------------------------------

    pub fn add_entity(&mut self, entity: Entity) -> Guid {
        let id = entity.id;
        self.entities.insert(id, entity);
        id
    }

    pub fn remove_entity(&mut self, id: &Guid) -> Option<Entity> {
        self.entities.remove(id)
    }

    pub fn get_entity(&self, id: &Guid) -> Option<&Entity> {
        self.entities.get(id)
    }

    pub fn get_entity_mut(&mut self, id: &Guid) -> Option<&mut Entity> {
        self.entities.get_mut(id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn define_block(
        &mut self,
        name: String,
        base_point: Vec3,
        entities: Vec<BlockEntity>,
        dynamic: Option<BlockDynamic>,
    ) -> bool {
        if name.trim().is_empty() || entities.is_empty() {
            return false;
        }
        let key = name.trim().to_ascii_lowercase();
        self.blocks.insert(
            key,
            BlockDefinition {
                name: name.trim().to_string(),
                base_point,
                entities,
                dynamic,
                dynamic_v1: None,
                cabinet_v1: None,
            },
        );
        true
    }

    pub fn get_block(&self, name: &str) -> Option<&BlockDefinition> {
        self.blocks.get(&name.trim().to_ascii_lowercase())
    }

    pub fn block_names(&self) -> Vec<String> {
        let mut out: Vec<String> = self.blocks.values().map(|b| b.name.clone()).collect();
        out.sort();
        out
    }

    pub fn export_block_to_file(&self, name: &str, path: &str) -> Result<()> {
        let block = self
            .get_block(name)
            .cloned()
            .ok_or_else(|| CadError::InvalidOperation(format!("Block '{}' not found", name)))?;
        let payload = BlockFileV1 {
            format: "cadkit-block".to_string(),
            version: 1,
            block,
        };
        let json = serde_json::to_string_pretty(&payload)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn import_block_from_file(&mut self, path: &str) -> Result<String> {
        let json = std::fs::read_to_string(path)?;
        let payload: BlockFileV1 = serde_json::from_str(&json)?;
        if payload.format != "cadkit-block" || payload.version != 1 {
            return Err(CadError::InvalidOperation(
                "Unsupported block file format".to_string(),
            ));
        }
        let key = payload.block.name.trim().to_ascii_lowercase();
        let name = payload.block.name.clone();
        self.blocks.insert(key, payload.block);
        Ok(name)
    }

    /// Fetch V1 dynamic block metadata for a block name.
    pub fn get_block_dynamic_v1(&self, name: &str) -> Option<&DynamicBlockDefinition> {
        self.get_block(name).and_then(|b| b.dynamic_v1.as_ref())
    }

    /// Set or clear V1 dynamic block metadata for a block name.
    pub fn set_block_dynamic_v1(
        &mut self,
        name: &str,
        dynamic_v1: Option<DynamicBlockDefinition>,
    ) -> bool {
        let key = name.trim().to_ascii_lowercase();
        let Some(block) = self.blocks.get_mut(&key) else {
            return false;
        };
        block.dynamic_v1 = dynamic_v1;
        true
    }

    /// Fetch V1 cabinet-definition metadata for a block name.
    pub fn get_block_cabinet_v1(&self, name: &str) -> Option<&CabinetDefinition> {
        self.get_block(name).and_then(|b| b.cabinet_v1.as_ref())
    }

    /// Set or clear V1 cabinet-definition metadata for a block name.
    pub fn set_block_cabinet_v1(
        &mut self,
        name: &str,
        cabinet_v1: Option<CabinetDefinition>,
    ) -> bool {
        let key = name.trim().to_ascii_lowercase();
        let Some(block) = self.blocks.get_mut(&key) else {
            return false;
        };
        block.cabinet_v1 = cabinet_v1;
        true
    }

    /// Return effective dynamic parameter values for an INSERT entity id.
    /// Defaults come from block `dynamic_v1.parameters`; entity overrides replace defaults.
    pub fn get_insert_effective_dynamic_params(
        &self,
        insert_id: &Guid,
    ) -> Option<HashMap<Guid, f64>> {
        let entity = self.get_entity(insert_id)?;
        let EntityKind::Insert { name, .. } = &entity.kind else {
            return None;
        };

        let mut values: HashMap<Guid, f64> = HashMap::new();
        if let Some(block_dyn) = self.get_block_dynamic_v1(name) {
            for p in &block_dyn.parameters {
                values.insert(p.id, p.default_value);
            }
        }
        for (pid, val) in &entity.insert_dynamic_param_overrides {
            values.insert(*pid, *val);
        }
        Some(values)
    }

    /// Return effective cabinet parameter values for an INSERT entity id.
    /// Defaults come from block `cabinet_v1.parameters`; entity overrides replace defaults.
    pub fn get_insert_effective_cabinet_params(
        &self,
        insert_id: &Guid,
    ) -> Result<HashMap<String, CabinetFormulaValue>> {
        let entity = self
            .get_entity(insert_id)
            .ok_or(CadError::NotFound(*insert_id))?;
        let EntityKind::Insert { name, .. } = &entity.kind else {
            return Err(CadError::InvalidOperation(
                "Cabinet parameters are only available on INSERT entities".to_string(),
            ));
        };
        let cabinet = self.get_block_cabinet_v1(name).ok_or_else(|| {
            CadError::InvalidOperation(format!("Block '{name}' has no cabinet_v1 definition"))
        })?;

        let mut values: HashMap<String, CabinetFormulaValue> = HashMap::new();
        for param in &cabinet.parameters {
            let raw = entity
                .insert_cabinet_param_overrides
                .get(&param.name)
                .map(String::as_str)
                .unwrap_or(param.default_value.as_str());
            values.insert(
                param.name.clone(),
                parse_cabinet_parameter_value(param, raw)?,
            );
        }
        Ok(values)
    }

    /// Return raw override map for an INSERT entity id.
    pub fn get_insert_dynamic_param_overrides(
        &self,
        insert_id: &Guid,
    ) -> Option<&HashMap<Guid, f64>> {
        let entity = self.get_entity(insert_id)?;
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return None;
        }
        Some(&entity.insert_dynamic_param_overrides)
    }

    /// Return raw cabinet override map for an INSERT entity id.
    pub fn get_insert_cabinet_param_overrides(
        &self,
        insert_id: &Guid,
    ) -> Option<&HashMap<String, String>> {
        let entity = self.get_entity(insert_id)?;
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return None;
        }
        Some(&entity.insert_cabinet_param_overrides)
    }

    /// Set/update one dynamic parameter override for an INSERT entity.
    pub fn set_insert_dynamic_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_id: Guid,
        value: f64,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_dynamic_param_overrides
            .insert(parameter_id, value);
        true
    }

    /// Remove one dynamic parameter override for an INSERT entity.
    pub fn remove_insert_dynamic_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_id: &Guid,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_dynamic_param_overrides
            .remove(parameter_id)
            .is_some()
    }

    /// Clear all dynamic parameter overrides for an INSERT entity.
    pub fn clear_insert_dynamic_param_overrides(&mut self, insert_id: &Guid) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity.insert_dynamic_param_overrides.clear();
        true
    }

    /// Set/update one cabinet parameter override for an INSERT entity.
    pub fn set_insert_cabinet_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_name: impl Into<String>,
        value: impl Into<String>,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_cabinet_param_overrides
            .insert(parameter_name.into(), value.into());
        true
    }

    /// Remove one cabinet parameter override for an INSERT entity.
    pub fn remove_insert_cabinet_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_name: &str,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_cabinet_param_overrides
            .remove(parameter_name)
            .is_some()
    }

    /// Clear all cabinet parameter overrides for an INSERT entity.
    pub fn clear_insert_cabinet_param_overrides(&mut self, insert_id: &Guid) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity.insert_cabinet_param_overrides.clear();
        true
    }

    /// Snapshot helper for serialization/interchange of per-instance dynamic values.
    pub fn get_block_instance_dynamic_state(
        &self,
        insert_id: &Guid,
    ) -> Option<BlockInstanceDynamicState> {
        let entity = self.get_entity(insert_id)?;
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return None;
        }
        Some(BlockInstanceDynamicState {
            insert_entity_id: *insert_id,
            param_values: entity.insert_dynamic_param_overrides.clone(),
        })
    }

    /// Apply a serialized dynamic state payload to an INSERT entity.
    pub fn apply_block_instance_dynamic_state(
        &mut self,
        state: &BlockInstanceDynamicState,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(&state.insert_entity_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity.insert_dynamic_param_overrides = state.param_values.clone();
        true
    }

    /// Evaluate one cabinet insert into spreadsheet-style generated part rows.
    pub fn evaluate_insert_cabinet_parts(
        &self,
        insert_id: &Guid,
    ) -> Result<Vec<CabinetGeneratedPart>> {
        let entity = self
            .get_entity(insert_id)
            .ok_or(CadError::NotFound(*insert_id))?;
        let EntityKind::Insert { name, .. } = &entity.kind else {
            return Err(CadError::InvalidOperation(
                "Cabinet parts can only be generated from INSERT entities".to_string(),
            ));
        };
        let cabinet = self.get_block_cabinet_v1(name).ok_or_else(|| {
            CadError::InvalidOperation(format!("Block '{name}' has no cabinet_v1 definition"))
        })?;
        let context = self.get_insert_effective_cabinet_params(insert_id)?;

        let mut parts = Vec::new();
        for row in &cabinet.part_recipe {
            if !row.enabled {
                continue;
            }

            let quantity = evaluate_cabinet_formula(&row.qty_formula, &context)?
                .as_number(&format!("recipe '{}' qty_formula", row.part_name))?;
            if quantity <= 0.0 {
                continue;
            }

            let length = evaluate_cabinet_formula(&row.length_formula, &context)?
                .as_number(&format!("recipe '{}' length_formula", row.part_name))?;
            let width = evaluate_cabinet_formula(&row.width_formula, &context)?
                .as_number(&format!("recipe '{}' width_formula", row.part_name))?;
            let thickness = evaluate_cabinet_formula(&row.thickness_formula, &context)?
                .as_number(&format!("recipe '{}' thickness_formula", row.part_name))?;
            let core_material = evaluate_cabinet_formula(&row.core_material_formula, &context)?
                .as_text(&format!("recipe '{}' core_material_formula", row.part_name));
            let finish = evaluate_cabinet_formula(&row.finish_formula, &context)?
                .as_text(&format!("recipe '{}' finish_formula", row.part_name));

            parts.push(CabinetGeneratedPart {
                insert_entity_id: *insert_id,
                block_name: name.clone(),
                family_name: cabinet.family_name.clone(),
                family_kind: cabinet.family_kind.clone(),
                part_name: row.part_name.clone(),
                quantity,
                length,
                width,
                thickness,
                core_material,
                finish,
                face: row.face,
                grain: row.grain,
                notes: row.notes.clone(),
            });
        }

        Ok(parts)
    }

    /// Evaluate INSERT-local entities, regenerating from `dynamic_v1` authored geometry
    /// when available. Falls back to stored block entities for non-dynamic blocks.
    pub fn evaluate_insert_local_entities(&self, insert: &Entity) -> Option<Vec<BlockEntity>> {
        let EntityKind::Insert { name, .. } = &insert.kind else {
            return None;
        };
        let block = self.get_block(name)?;
        let Some(dynv1) = &block.dynamic_v1 else {
            return Some(block.entities.clone());
        };
        if dynv1.base_entities.is_empty() {
            return Some(block.entities.clone());
        }

        // Always regenerate from authored base entities to avoid cumulative distortion.
        let mut working: Vec<BlockEntity> = dynv1
            .base_entities
            .iter()
            .map(|be| BlockEntity {
                kind: be.kind.clone(),
                layer: be.layer,
                color: None,
                linetype: Linetype::Continuous,
                linetype_by_layer: true,
                linetype_scale: None,
            })
            .collect();

        let authored_bounds_by_id: HashMap<Guid, (f64, f64, f64, f64)> = dynv1
            .base_entities
            .iter()
            .filter_map(|e| Self::kind_bounds_local(&e.kind).map(|b| (e.local_entity_id, b)))
            .collect();
        let index_by_local_id: HashMap<Guid, usize> = dynv1
            .base_entities
            .iter()
            .enumerate()
            .map(|(i, e)| (e.local_entity_id, i))
            .collect();

        let mut effective = self
            .get_insert_effective_dynamic_params(&insert.id)
            .unwrap_or_default();
        for p in &dynv1.parameters {
            effective.entry(p.id).or_insert(p.default_value);
        }
        if let Ok(cabinet_values) = self.get_insert_effective_cabinet_params(&insert.id) {
            for p in &dynv1.parameters {
                if let Some(CabinetFormulaValue::Number(v)) = cabinet_values.get(&p.name) {
                    effective.insert(p.id, *v);
                }
            }
        }

        let base_min_x = dynv1.base_bounds.min.x;
        let base_min_y = dynv1.base_bounds.min.y;
        let base_w = (dynv1.base_bounds.max.x - dynv1.base_bounds.min.x).max(1e-9);
        let base_h = (dynv1.base_bounds.max.y - dynv1.base_bounds.min.y).max(1e-9);
        let mut cur_w = base_w;
        let mut cur_h = base_h;
        for p in &dynv1.parameters {
            let val = *effective.get(&p.id).unwrap_or(&p.default_value);
            match p.axis {
                ParameterAxis::X => cur_w = val.max(1e-9),
                ParameterAxis::Y => cur_h = val.max(1e-9),
            }
        }

        let resolve_frame_value = |frame: ReferenceFrame, axis: ParameterAxis| -> f64 {
            match (frame, axis) {
                (ReferenceFrame::BlockOrigin, ParameterAxis::X) => 0.0,
                (ReferenceFrame::BlockOrigin, ParameterAxis::Y) => 0.0,
                (ReferenceFrame::BoundsCenter, ParameterAxis::X) => base_min_x + cur_w * 0.5,
                (ReferenceFrame::BoundsCenter, ParameterAxis::Y) => base_min_y + cur_h * 0.5,
                (ReferenceFrame::LeftEdge, ParameterAxis::X) => base_min_x,
                (ReferenceFrame::RightEdge, ParameterAxis::X) => base_min_x + cur_w,
                (ReferenceFrame::BottomEdge, ParameterAxis::Y) => base_min_y,
                (ReferenceFrame::TopEdge, ParameterAxis::Y) => base_min_y + cur_h,
                // If the frame does not map cleanly to this axis, fallback to center.
                (_, ParameterAxis::X) => base_min_x + cur_w * 0.5,
                (_, ParameterAxis::Y) => base_min_y + cur_h * 0.5,
            }
        };

        let mut actions = dynv1.actions.clone();
        actions.sort_by_key(|a| a.order);
        for action in actions {
            let Some(param) = dynv1
                .parameters
                .iter()
                .find(|p| p.id == action.parameter_id)
            else {
                continue;
            };
            let cur = *effective.get(&param.id).unwrap_or(&param.default_value);
            let delta = cur - param.default_value;
            if delta.abs() <= 1e-12 && !matches!(action.action_kind, ActionKind::Anchor) {
                continue;
            }

            for target in &action.targets {
                let mut idxs: Vec<usize> = Vec::new();
                match &target.target {
                    TargetRef::Entity(id) => {
                        if let Some(&idx) = index_by_local_id.get(id) {
                            idxs.push(idx);
                        }
                    }
                    TargetRef::Group(group_id) => {
                        if let Some(group) = dynv1.groups.iter().find(|g| g.id == *group_id) {
                            for member in &group.members {
                                if let Some(&idx) = index_by_local_id.get(member) {
                                    idxs.push(idx);
                                }
                            }
                        }
                    }
                    TargetRef::SubEntity { .. } => {
                        // TODO(dynamic-v1): Support sub-entity target deformation.
                        continue;
                    }
                }
                if idxs.is_empty() {
                    continue;
                }

                for idx in idxs {
                    let Some((x0, y0, x1, y1)) = Self::kind_bounds_local(&working[idx].kind) else {
                        continue;
                    };
                    let cx = (x0 + x1) * 0.5;
                    let cy = (y0 + y1) * 0.5;

                    let local_id = dynv1.base_entities[idx].local_entity_id;
                    let (default_off_x, default_off_y) = authored_bounds_by_id
                        .get(&local_id)
                        .map(|(ax0, ay0, ax1, ay1)| {
                            let acx = (ax0 + ax1) * 0.5;
                            let acy = (ay0 + ay1) * 0.5;
                            (
                                acx - resolve_frame_value(target.reference_frame, ParameterAxis::X),
                                acy - resolve_frame_value(target.reference_frame, ParameterAxis::Y),
                            )
                        })
                        .unwrap_or((0.0, 0.0));

                    let weight = if target.weight.is_finite() {
                        target.weight
                    } else {
                        1.0
                    };
                    let mut dx = 0.0;
                    let mut dy = 0.0;

                    match target.behavior {
                        EntityBehavior::MoveRigid => match param.axis {
                            ParameterAxis::X => dx = delta * weight,
                            ParameterAxis::Y => dy = delta * weight,
                        },
                        EntityBehavior::KeepCentered | EntityBehavior::AnchorToCenter => {
                            if target.axis_mask.x {
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => 0.0,
                                    PlacementRule::Proportional(_) => 0.0,
                                };
                                let center_x = resolve_frame_value(
                                    ReferenceFrame::BoundsCenter,
                                    ParameterAxis::X,
                                );
                                dx = (center_x + off) - cx;
                            }
                            if target.axis_mask.y {
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => 0.0,
                                    PlacementRule::Proportional(_) => 0.0,
                                };
                                let center_y = resolve_frame_value(
                                    ReferenceFrame::BoundsCenter,
                                    ParameterAxis::Y,
                                );
                                dy = (center_y + off) - cy;
                            }
                        }
                        EntityBehavior::AnchorToEdge => {
                            if target.axis_mask.x {
                                let ref_x =
                                    resolve_frame_value(target.reference_frame, ParameterAxis::X);
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => default_off_x,
                                    PlacementRule::Proportional(_) => default_off_x,
                                };
                                let edge_x = match target.reference_frame {
                                    ReferenceFrame::LeftEdge => x0,
                                    ReferenceFrame::RightEdge => x1,
                                    _ => cx,
                                };
                                dx = (ref_x + off) - edge_x;
                            }
                            if target.axis_mask.y {
                                let ref_y =
                                    resolve_frame_value(target.reference_frame, ParameterAxis::Y);
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => default_off_y,
                                    PlacementRule::Proportional(_) => default_off_y,
                                };
                                let edge_y = match target.reference_frame {
                                    ReferenceFrame::BottomEdge => y0,
                                    ReferenceFrame::TopEdge => y1,
                                    _ => cy,
                                };
                                dy = (ref_y + off) - edge_y;
                            }
                        }
                        EntityBehavior::StretchFromLeft
                        | EntityBehavior::StretchFromRight
                        | EntityBehavior::StretchFromCenter => {
                            let delta_units = delta * weight;
                            working[idx].kind = Self::stretch_kind_local(
                                &working[idx].kind,
                                param.axis,
                                target.behavior,
                                delta_units,
                                (x0, y0, x1, y1),
                            );
                            continue;
                        }
                        EntityBehavior::Ignore => continue,
                    }

                    if !target.axis_mask.x {
                        dx = 0.0;
                    }
                    if !target.axis_mask.y {
                        dy = 0.0;
                    }
                    if dx.abs() <= 1e-12 && dy.abs() <= 1e-12 {
                        continue;
                    }
                    working[idx].kind = Self::translate_kind_local(&working[idx].kind, dx, dy);
                }
            }
        }

        Some(working)
    }

    fn kind_bounds_local(kind: &EntityKind) -> Option<(f64, f64, f64, f64)> {
        match kind {
            EntityKind::Line { start, end } => Some((
                start.x.min(end.x),
                start.y.min(end.y),
                start.x.max(end.x),
                start.y.max(end.y),
            )),
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let mut pts: Vec<Vec2> = Vec::with_capacity(6);
                pts.push(Vec2::new(
                    center.x + radius * start_angle.cos(),
                    center.y + radius * start_angle.sin(),
                ));
                pts.push(Vec2::new(
                    center.x + radius * end_angle.cos(),
                    center.y + radius * end_angle.sin(),
                ));
                for &a in &[
                    0.0_f64,
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    3.0 * std::f64::consts::FRAC_PI_2,
                ] {
                    if Self::angle_in_arc_ccw(a, *start_angle, *end_angle) {
                        pts.push(Vec2::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ));
                    }
                }
                Self::bounds_from_points(&pts)
            }
            EntityKind::Circle { center, radius } => Some((
                center.x - radius,
                center.y - radius,
                center.x + radius,
                center.y + radius,
            )),
            EntityKind::Polyline { vertices, .. } => {
                if vertices.is_empty() {
                    return None;
                }
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for v in vertices {
                    min_x = min_x.min(v.x);
                    min_y = min_y.min(v.y);
                    max_x = max_x.max(v.x);
                    max_y = max_y.max(v.y);
                }
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::DimAligned {
                start,
                end,
                text_pos,
                ..
            }
            | EntityKind::DimLinear {
                start,
                end,
                text_pos,
                ..
            } => Some((
                start.x.min(end.x).min(text_pos.x),
                start.y.min(end.y).min(text_pos.y),
                start.x.max(end.x).max(text_pos.x),
                start.y.max(end.y).max(text_pos.y),
            )),
            EntityKind::DimAngular {
                vertex, text_pos, ..
            } => Some((
                vertex.x.min(text_pos.x),
                vertex.y.min(text_pos.y),
                vertex.x.max(text_pos.x),
                vertex.y.max(text_pos.y),
            )),
            EntityKind::DimRadial {
                center,
                radius,
                leader_pt,
                ..
            } => Some((
                (center.x - radius).min(leader_pt.x),
                (center.y - radius).min(leader_pt.y),
                (center.x + radius).max(leader_pt.x),
                (center.y + radius).max(leader_pt.y),
            )),
            EntityKind::Text { position, .. } => {
                Some((position.x, position.y, position.x, position.y))
            }
            EntityKind::Insert { position, .. } => {
                Some((position.x, position.y, position.x, position.y))
            }
        }
    }

    fn bounds_from_points(points: &[Vec2]) -> Option<(f64, f64, f64, f64)> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        Some((min_x, min_y, max_x, max_y))
    }

    fn angle_in_arc_ccw(a: f64, start: f64, end: f64) -> bool {
        use std::f64::consts::TAU;
        let norm = |mut v: f64| {
            while v < 0.0 {
                v += TAU;
            }
            while v >= TAU {
                v -= TAU;
            }
            v
        };
        let a = norm(a);
        let s = norm(start);
        let mut e = norm(end);
        if e < s {
            e += TAU;
        }
        let mut aa = a;
        if aa < s {
            aa += TAU;
        }
        aa >= s - 1e-12 && aa <= e + 1e-12
    }

    fn translate_kind_local(kind: &EntityKind, dx: f64, dy: f64) -> EntityKind {
        let tp = |p: Vec3| Vec3::xy(p.x + dx, p.y + dy);
        match kind {
            EntityKind::Line { start, end } => EntityKind::Line {
                start: tp(*start),
                end: tp(*end),
            },
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => EntityKind::Arc {
                center: tp(*center),
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: tp(*center),
                radius: *radius,
            },
            EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                vertices: vertices.iter().map(|v| tp(*v)).collect(),
                closed: *closed,
            },
            EntityKind::DimAligned {
                start,
                end,
                offset,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAligned {
                start: tp(*start),
                end: tp(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimLinear {
                start,
                end,
                offset,
                text_override,
                text_pos,
                horizontal,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimLinear {
                start: tp(*start),
                end: tp(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                horizontal: *horizontal,
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimAngular {
                vertex,
                line1_pt,
                line2_pt,
                radius,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAngular {
                vertex: tp(*vertex),
                line1_pt: tp(*line1_pt),
                line2_pt: tp(*line2_pt),
                radius: *radius,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimRadial {
                center,
                radius,
                leader_pt,
                is_diameter,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimRadial {
                center: tp(*center),
                radius: *radius,
                leader_pt: tp(*leader_pt),
                is_diameter: *is_diameter,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::Text {
                position,
                content,
                height,
                rotation,
                font_name,
            } => EntityKind::Text {
                position: tp(*position),
                content: content.clone(),
                height: *height,
                rotation: *rotation,
                font_name: font_name.clone(),
            },
            EntityKind::Insert {
                name,
                position,
                rotation,
                scale_x,
                scale_y,
            } => EntityKind::Insert {
                name: name.clone(),
                position: tp(*position),
                rotation: *rotation,
                scale_x: *scale_x,
                scale_y: *scale_y,
            },
        }
    }

    fn stretch_kind_local(
        kind: &EntityKind,
        axis: ParameterAxis,
        behavior: EntityBehavior,
        delta: f64,
        bounds: (f64, f64, f64, f64),
    ) -> EntityKind {
        let (x0, y0, x1, y1) = bounds;
        let cx = (x0 + x1) * 0.5;
        let cy = (y0 + y1) * 0.5;
        let tol_x = ((x1 - x0).abs()).max(1.0) * 1e-6;
        let tol_y = ((y1 - y0).abs()).max(1.0) * 1e-6;
        let stretch_point = |p: Vec3| -> Vec3 {
            let mut out = p;
            match (axis, behavior) {
                (ParameterAxis::X, EntityBehavior::StretchFromRight) => {
                    if (p.x - x1).abs() <= tol_x {
                        out.x += delta;
                    }
                }
                (ParameterAxis::X, EntityBehavior::StretchFromLeft) => {
                    if (p.x - x0).abs() <= tol_x {
                        out.x -= delta;
                    }
                }
                (ParameterAxis::X, EntityBehavior::StretchFromCenter) => {
                    if p.x > cx + tol_x {
                        out.x += delta * 0.5;
                    } else if p.x < cx - tol_x {
                        out.x -= delta * 0.5;
                    }
                }
                (ParameterAxis::Y, EntityBehavior::StretchFromRight) => {
                    if (p.y - y1).abs() <= tol_y {
                        out.y += delta;
                    }
                }
                (ParameterAxis::Y, EntityBehavior::StretchFromLeft) => {
                    if (p.y - y0).abs() <= tol_y {
                        out.y -= delta;
                    }
                }
                (ParameterAxis::Y, EntityBehavior::StretchFromCenter) => {
                    if p.y > cy + tol_y {
                        out.y += delta * 0.5;
                    } else if p.y < cy - tol_y {
                        out.y -= delta * 0.5;
                    }
                }
                _ => {}
            }
            out
        };

        match kind {
            EntityKind::Line { start, end } => EntityKind::Line {
                start: stretch_point(*start),
                end: stretch_point(*end),
            },
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => EntityKind::Arc {
                center: stretch_point(*center),
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: stretch_point(*center),
                radius: *radius,
            },
            EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                vertices: vertices.iter().map(|v| stretch_point(*v)).collect(),
                closed: *closed,
            },
            EntityKind::DimAligned {
                start,
                end,
                offset,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAligned {
                start: stretch_point(*start),
                end: stretch_point(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: stretch_point(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimLinear {
                start,
                end,
                offset,
                text_override,
                text_pos,
                horizontal,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimLinear {
                start: stretch_point(*start),
                end: stretch_point(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: stretch_point(*text_pos),
                horizontal: *horizontal,
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimAngular {
                vertex,
                line1_pt,
                line2_pt,
                radius,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAngular {
                vertex: stretch_point(*vertex),
                line1_pt: stretch_point(*line1_pt),
                line2_pt: stretch_point(*line2_pt),
                radius: *radius,
                text_override: text_override.clone(),
                text_pos: stretch_point(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimRadial {
                center,
                radius,
                leader_pt,
                is_diameter,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimRadial {
                center: stretch_point(*center),
                radius: *radius,
                leader_pt: stretch_point(*leader_pt),
                is_diameter: *is_diameter,
                text_override: text_override.clone(),
                text_pos: stretch_point(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::Text {
                position,
                content,
                height,
                rotation,
                font_name,
            } => EntityKind::Text {
                position: stretch_point(*position),
                content: content.clone(),
                height: *height,
                rotation: *rotation,
                font_name: font_name.clone(),
            },
            EntityKind::Insert {
                name,
                position,
                rotation,
                scale_x,
                scale_y,
            } => EntityKind::Insert {
                name: name.clone(),
                position: stretch_point(*position),
                rotation: *rotation,
                scale_x: *scale_x,
                scale_y: *scale_y,
            },
        }
    }

    // -------------------------------------------------------------------------
    // Layer Management
    // -------------------------------------------------------------------------

    /// Add a layer, automatically selecting the next palette colour.
    pub fn add_layer(&mut self, name: String) -> u32 {
        let color = LAYER_COLORS[self.next_layer_id as usize % LAYER_COLORS.len()];
        self.add_layer_with_color(name, color)
    }

    /// Add a layer with an explicit RGB colour.
    pub fn add_layer_with_color(&mut self, name: String, color: [u8; 3]) -> u32 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer = Layer::new(id, name, color);
        self.layers.insert(id, layer);
        id
    }

    /// Remove a layer by id. Returns false if the layer does not exist or has entities on it.
    /// Layer 0 cannot be removed.
    pub fn remove_layer(&mut self, id: u32) -> bool {
        if id == 0 {
            return false;
        }
        if self.entities.values().any(|e| e.layer == id) {
            return false;
        }
        self.layers.remove(&id).is_some()
    }

    pub fn get_layer(&self, id: u32) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    pub fn entities_on_layer(&self, layer_id: u32) -> impl Iterator<Item = &Entity> {
        self.entities.values().filter(move |e| e.layer == layer_id)
    }

    pub fn visible_entities(&self) -> impl Iterator<Item = &Entity> + '_ {
        self.entities.values().filter(|e| {
            self.layers
                .get(&e.layer)
                .map(|l| l.visible && !l.frozen)
                .unwrap_or(false)
        })
    }

    // -------------------------------------------------------------------------
    // File I/O
    // -------------------------------------------------------------------------

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let drawing = serde_json::from_str(&json)?;
        Ok(drawing)
    }
}

impl Default for Drawing {
    fn default() -> Self {
        Self::new("Untitled".to_string())
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a line entity on default layer
pub fn create_line(start: Vec2, end: Vec2) -> Entity {
    Entity::new(
        EntityKind::Line {
            start: start.into(),
            end: end.into(),
        },
        0,
    )
}

/// Create a circle entity on default layer
pub fn create_circle(center: Vec2, radius: f64) -> Entity {
    Entity::new(
        EntityKind::Circle {
            center: center.into(),
            radius,
        },
        0,
    )
}

/// Create an arc entity on default layer
pub fn create_arc(center: Vec2, radius: f64, start_angle: f64, end_angle: f64) -> Entity {
    Entity::new(
        EntityKind::Arc {
            center: center.into(),
            radius,
            start_angle,
            end_angle,
        },
        0,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_drawing() {
        let drawing = Drawing::new("Test".to_string());
        assert_eq!(drawing.name, "Test");
        assert_eq!(drawing.entity_count(), 0);
        assert_eq!(drawing.layers().count(), 1); // default layer
    }

    #[test]
    fn test_add_entity() {
        let mut drawing = Drawing::default();
        let line = create_line(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let id = drawing.add_entity(line);

        assert_eq!(drawing.entity_count(), 1);
        assert!(drawing.get_entity(&id).is_some());
    }

    #[test]
    fn test_remove_entity() {
        let mut drawing = Drawing::default();
        let line = create_line(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let id = drawing.add_entity(line);

        let removed = drawing.remove_entity(&id);
        assert!(removed.is_some());
        assert_eq!(drawing.entity_count(), 0);
    }

    #[test]
    fn test_layers() {
        let mut drawing = Drawing::default();
        let layer_id = drawing.add_layer("Walls".to_string());

        assert!(drawing.get_layer(layer_id).is_some());
        assert_eq!(drawing.get_layer(layer_id).unwrap().name, "Walls");
    }

    #[test]
    fn test_entity_is_planar() {
        let line = EntityKind::Line {
            start: Vec3::xy(0.0, 0.0),
            end: Vec3::xy(10.0, 10.0),
        };
        assert!(line.is_planar());

        let line_3d = EntityKind::Line {
            start: Vec3::new(0.0, 0.0, 5.0),
            end: Vec3::new(10.0, 10.0, 0.0),
        };
        assert!(!line_3d.is_planar());
    }

    #[test]
    fn test_save_load() {
        let mut drawing = Drawing::new("SaveTest".to_string());
        let line = create_line(Vec2::ZERO, Vec2::new(100.0, 50.0));
        drawing.add_entity(line);

        let temp_path = "/tmp/test_drawing.json";
        drawing.save_to_file(temp_path).unwrap();

        let loaded = Drawing::load_from_file(temp_path).unwrap();
        assert_eq!(loaded.name, "SaveTest");
        assert_eq!(loaded.entity_count(), 1);

        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_set_and_get_block_cabinet_v1() {
        let mut drawing = Drawing::default();
        assert!(drawing.define_block(
            "BaseCab".to_string(),
            Vec3::ZERO,
            vec![BlockEntity {
                kind: EntityKind::Line {
                    start: Vec3::xy(0.0, 0.0),
                    end: Vec3::xy(24.0, 0.0),
                },
                layer: 0,
                color: None,
                linetype: Linetype::Continuous,
                linetype_by_layer: false,
                linetype_scale: None,
            }],
            None,
        ));

        let cabinet = CabinetDefinition {
            family_name: "Base Cabinet".to_string(),
            family_kind: Some("base".to_string()),
            geometry_authored: false,
            parameters: vec![
                CabinetParameterDefinition {
                    id: Guid::new(),
                    name: "W".to_string(),
                    label: "Width".to_string(),
                    param_type: CabinetParameterType::Number,
                    default_value: "36".to_string(),
                    choice_options: Vec::new(),
                    unit: Some("in".to_string()),
                    notes: None,
                },
                CabinetParameterDefinition {
                    id: Guid::new(),
                    name: "EXPOSED_RIGHT".to_string(),
                    label: "Exposed Right".to_string(),
                    param_type: CabinetParameterType::Boolean,
                    default_value: "false".to_string(),
                    choice_options: Vec::new(),
                    unit: None,
                    notes: None,
                },
            ],
            views: vec![CabinetViewDefinition {
                kind: CabinetViewKind::Plan,
                description: Some("Plan footprint".to_string()),
                entity_ids: Vec::new(),
            }],
            part_recipe: vec![CabinetPartRecipeRow {
                id: Guid::new(),
                part_name: "SIDE_R".to_string(),
                enabled: true,
                qty_formula: "1".to_string(),
                length_formula: "H - TKH".to_string(),
                width_formula: "D".to_string(),
                thickness_formula: "THK".to_string(),
                core_material_formula: "COREMAT".to_string(),
                finish_formula: "if(EXPOSED_RIGHT, LAMCODE, \"\")".to_string(),
                face: Some(CabinetPartFace::Outside),
                grain: Some(CabinetGrainDirection::AlongLength),
                notes: Some("Right side panel".to_string()),
            }],
            notes: Some("Starter base cabinet".to_string()),
        };

        assert!(drawing.set_block_cabinet_v1("basecab", Some(cabinet)));
        let stored = drawing.get_block_cabinet_v1("BASECAB").unwrap();
        assert_eq!(stored.family_name, "Base Cabinet");
        assert_eq!(stored.part_recipe.len(), 1);
        assert_eq!(stored.part_recipe[0].part_name, "SIDE_R");
    }

    #[test]
    fn test_cabinet_v1_field_defaults_when_missing() {
        let json = r#"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "Legacy",
          "linetype_scale": 1.0,
          "entities": {},
          "blocks": {
            "testblock": {
              "name": "TestBlock",
              "base_point": {"x":0.0,"y":0.0,"z":0.0},
              "entities": []
            }
          },
          "layers": {
            "0": {
              "id": 0,
              "name": "0",
              "visible": true,
              "locked": false,
              "frozen": false,
              "color": [255,255,255],
              "linetype": "Continuous",
              "linetype_scale": 1.0
            }
          },
          "next_layer_id": 1
        }
        "#;
        let drawing: Drawing = serde_json::from_str(json).unwrap();
        let block = drawing.get_block("testblock").unwrap();
        assert!(block.cabinet_v1.is_none());
    }

    #[test]
    fn test_insert_cabinet_param_overrides_and_effective_values() {
        let mut drawing = Drawing::default();
        assert!(drawing.define_block(
            "BaseCab".to_string(),
            Vec3::ZERO,
            vec![BlockEntity {
                kind: EntityKind::Line {
                    start: Vec3::xy(0.0, 0.0),
                    end: Vec3::xy(24.0, 0.0),
                },
                layer: 0,
                color: None,
                linetype: Linetype::Continuous,
                linetype_by_layer: false,
                linetype_scale: None,
            }],
            None,
        ));
        assert!(drawing.set_block_cabinet_v1(
            "BaseCab",
            Some(CabinetDefinition {
                family_name: "Base Cabinet".to_string(),
                family_kind: Some("base".to_string()),
                geometry_authored: false,
                parameters: vec![
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "W".to_string(),
                        label: "Width".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "36".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "EXPOSED_RIGHT".to_string(),
                        label: "Exposed Right".to_string(),
                        param_type: CabinetParameterType::Boolean,
                        default_value: "false".to_string(),
                        choice_options: Vec::new(),
                        unit: None,
                        notes: None,
                    },
                ],
                views: Vec::new(),
                part_recipe: Vec::new(),
                notes: None,
            }),
        ));

        let insert = Entity::new(
            EntityKind::Insert {
                name: "BaseCab".to_string(),
                position: Vec3::ZERO,
                rotation: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
            },
            0,
        );
        let insert_id = drawing.add_entity(insert);
        assert!(drawing.set_insert_cabinet_param_override(&insert_id, "W", "42"));
        assert!(drawing.set_insert_cabinet_param_override(&insert_id, "EXPOSED_RIGHT", "true"));

        let values = drawing
            .get_insert_effective_cabinet_params(&insert_id)
            .unwrap();
        assert_eq!(values.get("W"), Some(&CabinetFormulaValue::Number(42.0)));
        assert_eq!(
            values.get("EXPOSED_RIGHT"),
            Some(&CabinetFormulaValue::Boolean(true))
        );

        assert!(drawing.remove_insert_cabinet_param_override(&insert_id, "W"));
        let values = drawing
            .get_insert_effective_cabinet_params(&insert_id)
            .unwrap();
        assert_eq!(values.get("W"), Some(&CabinetFormulaValue::Number(36.0)));
    }

    #[test]
    fn test_evaluate_insert_cabinet_parts() {
        let mut drawing = Drawing::default();
        assert!(drawing.define_block(
            "BaseCab".to_string(),
            Vec3::ZERO,
            vec![BlockEntity {
                kind: EntityKind::Line {
                    start: Vec3::xy(0.0, 0.0),
                    end: Vec3::xy(24.0, 0.0),
                },
                layer: 0,
                color: None,
                linetype: Linetype::Continuous,
                linetype_by_layer: false,
                linetype_scale: None,
            }],
            None,
        ));
        assert!(drawing.set_block_cabinet_v1(
            "BaseCab",
            Some(CabinetDefinition {
                family_name: "Base Cabinet".to_string(),
                family_kind: Some("base".to_string()),
                geometry_authored: false,
                parameters: vec![
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "W".to_string(),
                        label: "Width".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "36".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "H".to_string(),
                        label: "Height".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "34.5".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "D".to_string(),
                        label: "Depth".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "24".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "TKH".to_string(),
                        label: "Toe Kick Height".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "4".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "THK".to_string(),
                        label: "Panel Thickness".to_string(),
                        param_type: CabinetParameterType::Number,
                        default_value: "0.75".to_string(),
                        choice_options: Vec::new(),
                        unit: Some("in".to_string()),
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "COREMAT".to_string(),
                        label: "Core Material".to_string(),
                        param_type: CabinetParameterType::Text,
                        default_value: "PB_3_4_WHITE".to_string(),
                        choice_options: Vec::new(),
                        unit: None,
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "LAMCODE".to_string(),
                        label: "Laminate Code".to_string(),
                        param_type: CabinetParameterType::Text,
                        default_value: "WHT".to_string(),
                        choice_options: Vec::new(),
                        unit: None,
                        notes: None,
                    },
                    CabinetParameterDefinition {
                        id: Guid::new(),
                        name: "EXPOSED_RIGHT".to_string(),
                        label: "Exposed Right".to_string(),
                        param_type: CabinetParameterType::Boolean,
                        default_value: "false".to_string(),
                        choice_options: Vec::new(),
                        unit: None,
                        notes: None,
                    },
                ],
                views: Vec::new(),
                part_recipe: vec![
                    CabinetPartRecipeRow {
                        id: Guid::new(),
                        part_name: "SIDE_R".to_string(),
                        enabled: true,
                        qty_formula: "1".to_string(),
                        length_formula: "H - TKH".to_string(),
                        width_formula: "D".to_string(),
                        thickness_formula: "THK".to_string(),
                        core_material_formula: "COREMAT".to_string(),
                        finish_formula: "if(EXPOSED_RIGHT, LAMCODE, \"\")".to_string(),
                        face: Some(CabinetPartFace::Outside),
                        grain: Some(CabinetGrainDirection::AlongLength),
                        notes: Some("Right side".to_string()),
                    },
                    CabinetPartRecipeRow {
                        id: Guid::new(),
                        part_name: "STRETCHER".to_string(),
                        enabled: true,
                        qty_formula: "max(0, 2)".to_string(),
                        length_formula: "W - 2 * THK".to_string(),
                        width_formula: "4".to_string(),
                        thickness_formula: "THK".to_string(),
                        core_material_formula: "COREMAT".to_string(),
                        finish_formula: "\"\"".to_string(),
                        face: None,
                        grain: Some(CabinetGrainDirection::AlongLength),
                        notes: None,
                    },
                ],
                notes: None,
            }),
        ));

        let insert = Entity::new(
            EntityKind::Insert {
                name: "BaseCab".to_string(),
                position: Vec3::ZERO,
                rotation: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
            },
            0,
        );
        let insert_id = drawing.add_entity(insert);
        assert!(drawing.set_insert_cabinet_param_override(&insert_id, "W", "42"));
        assert!(drawing.set_insert_cabinet_param_override(&insert_id, "EXPOSED_RIGHT", "true"));

        let parts = drawing.evaluate_insert_cabinet_parts(&insert_id).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].part_name, "SIDE_R");
        assert_eq!(parts[0].quantity, 1.0);
        assert_eq!(parts[0].length, 30.5);
        assert_eq!(parts[0].width, 24.0);
        assert_eq!(parts[0].thickness, 0.75);
        assert_eq!(parts[0].core_material, "PB_3_4_WHITE");
        assert_eq!(parts[0].finish, "WHT");
        assert_eq!(parts[1].part_name, "STRETCHER");
        assert_eq!(parts[1].quantity, 2.0);
        assert!((parts[1].length - 40.5).abs() < 1e-9);
    }

    #[test]
    fn test_cabinet_formula_comparisons_and_empty_formula() {
        let mut context = HashMap::new();
        context.insert("W".to_string(), CabinetFormulaValue::Number(42.0));
        context.insert(
            "FINISH".to_string(),
            CabinetFormulaValue::Text("MAPLE".to_string()),
        );

        assert_eq!(
            evaluate_cabinet_formula("W >= 42", &context).unwrap(),
            CabinetFormulaValue::Boolean(true)
        );
        assert_eq!(
            evaluate_cabinet_formula("FINISH == \"MAPLE\"", &context).unwrap(),
            CabinetFormulaValue::Boolean(true)
        );
        assert_eq!(
            evaluate_cabinet_formula("", &context).unwrap(),
            CabinetFormulaValue::Text(String::new())
        );
    }
}
