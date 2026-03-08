use anyhow::{anyhow, Result};
use cadkit_2d_core::{create_arc, create_circle, create_line, Drawing, Entity, EntityKind};
use cadkit_types::{Vec2, Vec3};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyTuple};

#[pyclass(name = "Cad", skip_from_py_object)]
#[derive(Clone)]
pub struct CadPy {
    drawing: Drawing,
    current_layer: u32,
}

impl CadPy {
    fn with_drawing(drawing: Drawing) -> Self {
        Self {
            drawing,
            current_layer: 0,
        }
    }

    fn apply_layer(&self, entity: &mut cadkit_2d_core::Entity) {
        entity.layer = self.current_layer;
    }

    fn parse_point(obj: &Bound<'_, PyAny>) -> PyResult<(f64, f64)> {
        obj.extract::<(f64, f64)>()
            .map_err(|_| PyValueError::new_err("expected point tuple (x, y)"))
    }

    fn kw_f64(kwargs: Option<&Bound<'_, PyDict>>, key: &str) -> PyResult<Option<f64>> {
        let Some(kw) = kwargs else { return Ok(None) };
        match kw.get_item(key)? {
            Some(v) => Ok(Some(v.extract::<f64>()?)),
            None => Ok(None),
        }
    }

    fn kw_point(kwargs: Option<&Bound<'_, PyDict>>, key: &str) -> PyResult<Option<(f64, f64)>> {
        let Some(kw) = kwargs else { return Ok(None) };
        match kw.get_item(key)? {
            Some(v) => Ok(Some(Self::parse_point(&v)?)),
            None => Ok(None),
        }
    }
}

#[pymethods]
impl CadPy {
    #[new]
    fn py_new() -> Self {
        Self::with_drawing(Drawing::new("Python Script".to_string()))
    }

    /// Set the current target layer id for subsequent entities.
    fn set_layer(&mut self, layer_id: u32) -> PyResult<()> {
        if self.drawing.get_layer(layer_id).is_none() {
            return Err(PyValueError::new_err(format!(
                "Layer {} does not exist",
                layer_id
            )));
        }
        self.current_layer = layer_id;
        Ok(())
    }

    /// Create a line.
    /// Supports `line(x1,y1,x2,y2)` or `line((x1,y1),(x2,y2))`.
    /// Also supports keyword style: `line(start=(x1,y1), end=(x2,y2))`.
    #[pyo3(signature = (*args, **kwargs))]
    fn line(
        &mut self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let kwargs_xy = {
            let start = Self::kw_point(kwargs, "start")?;
            let end = Self::kw_point(kwargs, "end")?;
            let x1 = Self::kw_f64(kwargs, "x1")?;
            let y1 = Self::kw_f64(kwargs, "y1")?;
            let x2 = Self::kw_f64(kwargs, "x2")?;
            let y2 = Self::kw_f64(kwargs, "y2")?;
            if let (Some((sx, sy)), Some((ex, ey))) = (start, end) {
                Some((sx, sy, ex, ey))
            } else if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) {
                Some((x1, y1, x2, y2))
            } else {
                None
            }
        };
        let (x1, y1, x2, y2) = if let Some(xy) = kwargs_xy {
            xy
        } else {
            match args.len() {
                4 => (
                    args.get_item(0)?.extract::<f64>()?,
                    args.get_item(1)?.extract::<f64>()?,
                    args.get_item(2)?.extract::<f64>()?,
                    args.get_item(3)?.extract::<f64>()?,
                ),
                2 => {
                    let (x1, y1) = Self::parse_point(&args.get_item(0)?)?;
                    let (x2, y2) = Self::parse_point(&args.get_item(1)?)?;
                    (x1, y1, x2, y2)
                }
                _ => {
                    return Err(PyValueError::new_err(
                        "line expects (x1,y1,x2,y2), ((x1,y1),(x2,y2)), or keywords start/end",
                    ))
                }
            }
        };
        let mut line = create_line(Vec2::new(x1, y1), Vec2::new(x2, y2));
        self.apply_layer(&mut line);
        self.drawing.add_entity(line);
        Ok(())
    }

    /// Create a circle.
    /// Supports `circle(cx,cy,radius)` or `circle((cx,cy), radius)`.
    /// Also supports keyword style: `circle(center=(cx,cy), radius=r)`.
    #[pyo3(signature = (*args, **kwargs))]
    fn circle(
        &mut self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let kwargs_vals = {
            let center = Self::kw_point(kwargs, "center")?;
            let radius = Self::kw_f64(kwargs, "radius")?;
            let cx = Self::kw_f64(kwargs, "cx")?;
            let cy = Self::kw_f64(kwargs, "cy")?;
            if let (Some((cx, cy)), Some(r)) = (center, radius) {
                Some((cx, cy, r))
            } else if let (Some(cx), Some(cy), Some(r)) = (cx, cy, radius) {
                Some((cx, cy, r))
            } else {
                None
            }
        };
        let (cx, cy, radius) = if let Some(v) = kwargs_vals {
            v
        } else {
            match args.len() {
                3 => (
                    args.get_item(0)?.extract::<f64>()?,
                    args.get_item(1)?.extract::<f64>()?,
                    args.get_item(2)?.extract::<f64>()?,
                ),
                2 => {
                    let (cx, cy) = Self::parse_point(&args.get_item(0)?)?;
                    let radius = args.get_item(1)?.extract::<f64>()?;
                    (cx, cy, radius)
                }
                _ => {
                    return Err(PyValueError::new_err(
                        "circle expects (cx,cy,r), ((cx,cy),r), or keywords center/radius",
                    ))
                }
            }
        };
        if !(radius.is_finite() && radius > 0.0) {
            return Err(PyValueError::new_err("radius must be > 0"));
        }
        let mut circle = create_circle(Vec2::new(cx, cy), radius);
        self.apply_layer(&mut circle);
        self.drawing.add_entity(circle);
        Ok(())
    }

    /// Create an arc.
    /// Supports `arc(cx,cy,r,start_deg,end_deg)` or `arc((cx,cy),r,start_deg,end_deg)`.
    /// Also supports keyword style: `arc(center=(cx,cy), radius=r, start_deg=a, end_deg=b)`.
    #[pyo3(signature = (*args, **kwargs))]
    fn arc(
        &mut self,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let kwargs_vals = {
            let center = Self::kw_point(kwargs, "center")?;
            let radius = Self::kw_f64(kwargs, "radius")?;
            let start_deg =
                Self::kw_f64(kwargs, "start_deg")?.or(Self::kw_f64(kwargs, "start_angle")?);
            let end_deg = Self::kw_f64(kwargs, "end_deg")?.or(Self::kw_f64(kwargs, "end_angle")?);
            let cx = Self::kw_f64(kwargs, "cx")?;
            let cy = Self::kw_f64(kwargs, "cy")?;
            if let (Some((cx, cy)), Some(radius), Some(sa), Some(ea)) =
                (center, radius, start_deg, end_deg)
            {
                Some((cx, cy, radius, sa, ea))
            } else if let (Some(cx), Some(cy), Some(radius), Some(sa), Some(ea)) =
                (cx, cy, radius, start_deg, end_deg)
            {
                Some((cx, cy, radius, sa, ea))
            } else {
                None
            }
        };
        let (cx, cy, radius, start_deg, end_deg) = if let Some(v) = kwargs_vals {
            v
        } else {
            match args.len() {
                5 => (
                    args.get_item(0)?.extract::<f64>()?,
                    args.get_item(1)?.extract::<f64>()?,
                    args.get_item(2)?.extract::<f64>()?,
                    args.get_item(3)?.extract::<f64>()?,
                    args.get_item(4)?.extract::<f64>()?,
                ),
                4 => {
                    let (cx, cy) = Self::parse_point(&args.get_item(0)?)?;
                    let radius = args.get_item(1)?.extract::<f64>()?;
                    let start_deg = args.get_item(2)?.extract::<f64>()?;
                    let end_deg = args.get_item(3)?.extract::<f64>()?;
                    (cx, cy, radius, start_deg, end_deg)
                }
                _ => {
                    return Err(PyValueError::new_err(
                        "arc expects positional or keyword center/radius/start_deg/end_deg",
                    ))
                }
            }
        };
        if !(radius.is_finite() && radius > 0.0) {
            return Err(PyValueError::new_err("radius must be > 0"));
        }
        let start = start_deg.to_radians();
        let end = end_deg.to_radians();
        let mut arc = create_arc(Vec2::new(cx, cy), radius, start, end);
        self.apply_layer(&mut arc);
        self.drawing.add_entity(arc);
        Ok(())
    }

    /// Create a linear dimension.
    ///
    /// `horizontal=True` measures X distance; `horizontal=False` measures Y distance.
    /// `offset` is signed displacement of the dim line from the measured points.
    #[pyo3(signature = (x1, y1, x2, y2, offset, horizontal=true, text_override=None))]
    fn dim_linear(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        offset: f64,
        horizontal: bool,
        text_override: Option<String>,
    ) -> PyResult<()> {
        if !(offset.is_finite()
            && x1.is_finite()
            && y1.is_finite()
            && x2.is_finite()
            && y2.is_finite())
        {
            return Err(PyValueError::new_err("all inputs must be finite numbers"));
        }
        let text_pos = if horizontal {
            Vec3::xy((x1 + x2) * 0.5, (y1 + y2) * 0.5 + offset)
        } else {
            Vec3::xy((x1 + x2) * 0.5 + offset, (y1 + y2) * 0.5)
        };
        let mut dim = Entity::new(
            EntityKind::DimLinear {
                start: Vec3::xy(x1, y1),
                end: Vec3::xy(x2, y2),
                offset,
                text_override,
                text_pos,
                horizontal,
                arrow_length: 3.0,
                arrow_half_width: 0.75,
            },
            self.current_layer,
        );
        self.apply_layer(&mut dim);
        self.drawing.add_entity(dim);
        Ok(())
    }

    /// Number of entities currently in the script drawing.
    fn entity_count(&self) -> usize {
        self.drawing.entity_count()
    }

    /// Return entity IDs matching optional filters.
    ///
    /// Examples:
    /// - `cad.select()` -> all entity IDs
    /// - `cad.select(\"line\")`
    /// - `cad.select(layer=2)`
    #[pyo3(signature = (kind=None, layer=None))]
    fn select(&self, kind: Option<String>, layer: Option<u32>) -> Vec<String> {
        let kind = kind.map(|k| k.to_lowercase());
        self.drawing
            .entities()
            .filter(|e| {
                let layer_ok = layer.map(|l| e.layer == l).unwrap_or(true);
                let kind_ok = kind
                    .as_ref()
                    .map(|k| entity_kind_name(&e.kind) == k)
                    .unwrap_or(true);
                layer_ok && kind_ok
            })
            .map(|e| e.id.to_string())
            .collect()
    }

    /// Return a dictionary describing an entity by ID, or `None` if not found.
    fn get_entity(&self, py: Python<'_>, entity_id: &str) -> PyResult<Option<Py<PyDict>>> {
        let found = self
            .drawing
            .entities()
            .find(|e| e.id.to_string() == entity_id);
        let Some(e) = found else { return Ok(None) };
        let d = PyDict::new(py);
        d.set_item("id", e.id.to_string())?;
        d.set_item("kind", entity_kind_name(&e.kind))?;
        d.set_item("layer", e.layer)?;
        match e.color {
            Some([r, g, b]) => d.set_item("color", (r, g, b))?,
            None => d.set_item("color", py.None())?,
        }
        match &e.kind {
            EntityKind::Line { start, end } => {
                d.set_item("start", (start.x, start.y))?;
                d.set_item("end", (end.x, end.y))?;
            }
            EntityKind::Circle { center, radius } => {
                d.set_item("center", (center.x, center.y))?;
                d.set_item("radius", *radius)?;
            }
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                d.set_item("center", (center.x, center.y))?;
                d.set_item("radius", *radius)?;
                d.set_item("start_deg", start_angle.to_degrees())?;
                d.set_item("end_deg", end_angle.to_degrees())?;
            }
            EntityKind::Polyline { vertices, closed } => {
                let pts: Vec<(f64, f64)> = vertices.iter().map(|v| (v.x, v.y)).collect();
                d.set_item("vertices", pts)?;
                d.set_item("closed", *closed)?;
            }
            EntityKind::DimLinear {
                start,
                end,
                offset,
                text_override,
                text_pos,
                horizontal,
                ..
            } => {
                d.set_item("start", (start.x, start.y))?;
                d.set_item("end", (end.x, end.y))?;
                d.set_item("offset", *offset)?;
                d.set_item("horizontal", *horizontal)?;
                d.set_item("text_pos", (text_pos.x, text_pos.y))?;
                match text_override {
                    Some(s) => d.set_item("text_override", s)?,
                    None => d.set_item("text_override", py.None())?,
                }
            }
            _ => {}
        }
        Ok(Some(d.unbind()))
    }
}

/// Embedded Python runner for cad scripts.
///
/// Scripts receive a pre-bound `cad` object with methods:
/// - `cad.line(x1, y1, x2, y2)`
/// - `cad.circle(cx, cy, radius)`
/// - `cad.arc(cx, cy, radius, start_deg, end_deg)`
/// - `cad.dim_linear(x1, y1, x2, y2, offset, horizontal=True, text_override=None)`
pub struct PythonEngine;

impl PythonEngine {
    pub fn run_script_in_place(drawing: &mut Drawing, script: &str) -> Result<()> {
        Python::attach(|py| -> Result<()> {
            let cad_obj = Py::new(py, CadPy::with_drawing(drawing.clone()))
                .map_err(|e| anyhow!("Failed to create python cad bridge: {e}"))?;
            let globals = PyDict::new(py);
            globals
                .set_item("cad", cad_obj.clone_ref(py))
                .map_err(|e| anyhow!("Failed to bind python locals: {e}"))?;

            let builtins = py
                .import("builtins")
                .map_err(|e| anyhow!(format_python_error(py, e)))?;
            let exec_fn = builtins
                .getattr("exec")
                .map_err(|e| anyhow!(format_python_error(py, e)))?;
            exec_fn
                .call1((script, &globals, &globals))
                .map_err(|e| anyhow!(format_python_error(py, e)))?;

            let cad_ref = cad_obj.borrow(py);
            *drawing = cad_ref.drawing.clone();
            Ok(())
        })
    }

    pub fn run_script(drawing: &Drawing, script: &str) -> Result<Drawing> {
        let mut out = drawing.clone();
        Self::run_script_in_place(&mut out, script)?;
        Ok(out)
    }
}

fn entity_kind_name(kind: &EntityKind) -> &'static str {
    match kind {
        EntityKind::Line { .. } => "line",
        EntityKind::Circle { .. } => "circle",
        EntityKind::Arc { .. } => "arc",
        EntityKind::Polyline { .. } => "polyline",
        EntityKind::DimAligned { .. } => "dimaligned",
        EntityKind::DimLinear { .. } => "dimlinear",
        EntityKind::DimAngular { .. } => "dimangular",
        EntityKind::DimRadial { .. } => "dimradial",
        EntityKind::Text { .. } => "text",
        EntityKind::Insert { .. } => "insert",
    }
}

fn format_python_error(py: Python<'_>, err: PyErr) -> String {
    let value = err.value(py);
    let text = value
        .str()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "<unprintable python error>".to_string());
    if let Ok(t) = err.get_type(py).name() {
        format!("Python {}: {}", t, text)
    } else {
        format!("Python error: {}", text)
    }
}

#[pymodule]
fn cadkit_scripting_python(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadPy>()?;
    m.add("__doc__", "CadKit embedded Python bridge")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_bridge_creates_entities() {
        let mut drawing = Drawing::new("Test".to_string());
        let script = r#"
cad.line(0, 0, 10, 0)
cad.circle(5, 5, 2.5)
cad.arc(0, 0, 3, 0, 90)
"#;

        PythonEngine::run_script_in_place(&mut drawing, script).unwrap();
        assert_eq!(drawing.entity_count(), 3);
    }

    #[test]
    fn python_bridge_returns_error_for_bad_radius() {
        let drawing = Drawing::new("Test".to_string());
        let script = "cad.circle(0,0,0)";
        let err = PythonEngine::run_script(&drawing, script).unwrap_err();
        assert!(err.to_string().contains("radius must be > 0"));
    }

    #[test]
    fn python_bridge_query_api() {
        let mut drawing = Drawing::new("Test".to_string());
        let script = r#"
cad.line(0, 0, 10, 0)
cad.circle(5, 5, 2.5)
ids = cad.select("line")
assert len(ids) == 1
ent = cad.get_entity(ids[0])
assert ent is not None
assert ent["kind"] == "line"
assert ent["layer"] == 0
"#;
        PythonEngine::run_script_in_place(&mut drawing, script).unwrap();
    }

    #[test]
    fn python_bridge_dim_linear_api() {
        let mut drawing = Drawing::new("Test".to_string());
        let script = r#"
cad.dim_linear(0, 0, 24, 0, 5, True, "<>")
ids = cad.select("dimlinear")
assert len(ids) == 1
ent = cad.get_entity(ids[0])
assert ent["horizontal"] is True
assert abs(ent["offset"] - 5) < 1e-9
"#;
        PythonEngine::run_script_in_place(&mut drawing, script).unwrap();
    }
}
