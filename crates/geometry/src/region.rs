//! Basic closed-boundary detection for hatch/region workflows.
//!
//! First pass scope:
//! - Input is line segments (or polylines decomposed to line segments).
//! - Vertices are welded by tolerance.
//! - Closed loops are traced by walking unused edges.
//! - Output loops are returned as closed polylines.

use std::collections::HashSet;

use cadkit_types::Vec3;

use crate::{Intersection, Intersects, Line, Polyline};

#[derive(Clone, Debug)]
struct Node {
    p: Vec3,
}

#[derive(Clone, Debug)]
struct Edge {
    a: usize,
    b: usize,
}

fn sqr(x: f64) -> f64 {
    x * x
}

fn dist2(a: Vec3, b: Vec3) -> f64 {
    sqr(a.x - b.x) + sqr(a.y - b.y)
}

fn weld_or_push(nodes: &mut Vec<Node>, p: Vec3, tol2: f64) -> usize {
    if let Some((idx, _)) = nodes
        .iter()
        .enumerate()
        .find(|(_, n)| dist2(n.p, p) <= tol2)
    {
        return idx;
    }
    let idx = nodes.len();
    nodes.push(Node { p });
    idx
}

fn segment_param(line: &Line, p: Vec3, tol: f64) -> Option<f64> {
    let dx = line.end.x - line.start.x;
    let dy = line.end.y - line.start.y;
    let len2 = dx * dx + dy * dy;
    if len2 <= tol * tol {
        return None;
    }
    let t = if dx.abs() >= dy.abs() {
        if dx.abs() <= tol {
            return None;
        }
        (p.x - line.start.x) / dx
    } else {
        if dy.abs() <= tol {
            return None;
        }
        (p.y - line.start.y) / dy
    };
    if t < -tol || t > 1.0 + tol {
        return None;
    }
    Some(t.clamp(0.0, 1.0))
}

fn noded_segments(segments: &[Line], tol: f64) -> Vec<Line> {
    if segments.is_empty() {
        return Vec::new();
    }
    let n = segments.len();
    let mut cuts: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            match segments[i].intersect(&segments[j], tol) {
                Intersection::None | Intersection::Coincident => {}
                hit => {
                    for p in hit.points() {
                        if let Some(ti) = segment_param(&segments[i], p, tol) {
                            cuts[i].push(ti);
                        }
                        if let Some(tj) = segment_param(&segments[j], p, tol) {
                            cuts[j].push(tj);
                        }
                    }
                }
            }
        }
    }

    let mut out: Vec<Line> = Vec::new();
    for (idx, seg) in segments.iter().enumerate() {
        let mut ts = cuts[idx].clone();
        ts.sort_by(|a, b| a.total_cmp(b));
        ts.dedup_by(|a, b| (*a - *b).abs() <= tol);
        for pair in ts.windows(2) {
            let t0 = pair[0];
            let t1 = pair[1];
            if (t1 - t0).abs() <= tol {
                continue;
            }
            let p0 = seg.point_at(t0);
            let p1 = seg.point_at(t1);
            let split = Line::new(p0, p1);
            if !split.is_degenerate(tol) {
                out.push(split);
            }
        }
    }
    out
}

fn build_graph(segments: &[Line], tol: f64) -> (Vec<Node>, Vec<Edge>) {
    let tol2 = sqr(tol.max(1e-9));
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();

    for s in segments {
        if s.is_degenerate(tol) {
            continue;
        }
        let a = weld_or_push(&mut nodes, s.start, tol2);
        let b = weld_or_push(&mut nodes, s.end, tol2);
        if a == b {
            continue;
        }
        edges.push(Edge { a, b });
    }

    (nodes, edges)
}

/// Detect closed boundaries from line segments.
///
/// Returns one closed polyline per detected loop.
pub fn detect_closed_boundaries(segments: &[Line], tol: f64) -> Vec<Polyline> {
    let tol = tol.max(1e-9);
    let split = noded_segments(segments, tol);
    let (nodes, edges) = build_graph(&split, tol);
    if nodes.is_empty() || edges.is_empty() {
        return Vec::new();
    }

    #[derive(Clone, Copy)]
    struct HalfEdge {
        from: usize,
        to: usize,
        twin: usize,
        angle: f64,
    }

    // Build directed half-edges (two per undirected edge).
    let mut hes: Vec<HalfEdge> = Vec::with_capacity(edges.len() * 2);
    for e in &edges {
        let a = nodes[e.a].p;
        let b = nodes[e.b].p;
        let ab = (b.y - a.y).atan2(b.x - a.x);
        let ba = (a.y - b.y).atan2(a.x - b.x);
        let i0 = hes.len();
        let i1 = i0 + 1;
        hes.push(HalfEdge {
            from: e.a,
            to: e.b,
            twin: i1,
            angle: ab,
        });
        hes.push(HalfEdge {
            from: e.b,
            to: e.a,
            twin: i0,
            angle: ba,
        });
    }

    // Outgoing half-edges per node sorted CCW by angle.
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (hid, he) in hes.iter().enumerate() {
        outgoing[he.from].push(hid);
    }
    for list in &mut outgoing {
        list.sort_by(|&a, &b| hes[a].angle.total_cmp(&hes[b].angle));
    }

    // Face-walk next relation:
    // next(he) = edge just clockwise from twin(he) in outgoing list at he.to.
    let mut next = vec![usize::MAX; hes.len()];
    for (hid, he) in hes.iter().enumerate() {
        let list = &outgoing[he.to];
        if list.is_empty() {
            continue;
        }
        let twin = he.twin;
        let Some(pos) = list.iter().position(|&x| x == twin) else {
            continue;
        };
        let idx = if pos == 0 { list.len() - 1 } else { pos - 1 };
        next[hid] = list[idx];
    }

    let mut visited = vec![false; hes.len()];
    let mut loops: Vec<Polyline> = Vec::new();
    let mut loop_keys: HashSet<String> = HashSet::new();

    for start in 0..hes.len() {
        if visited[start] || next[start] == usize::MAX {
            continue;
        }
        let mut walk: Vec<usize> = Vec::new();
        let mut cur = start;
        let mut ok = false;
        let max_steps = hes.len().saturating_mul(2).max(16);
        for _ in 0..max_steps {
            if visited[cur] {
                break;
            }
            visited[cur] = true;
            walk.push(hes[cur].from);
            let nxt = next[cur];
            if nxt == usize::MAX {
                break;
            }
            cur = nxt;
            if cur == start {
                ok = true;
                break;
            }
        }
        if !ok || walk.len() < 3 {
            continue;
        }

        let verts: Vec<Vec3> = walk.iter().map(|&nid| nodes[nid].p).collect();
        let key = canonical_loop_key(&verts);
        if loop_keys.contains(&key) {
            continue;
        }
        loop_keys.insert(key);
        loops.push(Polyline::new(verts, true));
    }

    // Keep interior faces: positive signed area under this walk convention.
    let mut interiors: Vec<Polyline> = loops
        .into_iter()
        .filter(|p| signed_polygon_area(&p.vertices) > tol * tol)
        .collect();
    // Fallback if convention differs in some graphs.
    if interiors.is_empty() {
        interiors = Vec::new();
    }
    interiors
}

fn signed_polygon_area(vertices: &[Vec3]) -> f64 {
    if vertices.len() < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..vertices.len() {
        let a = vertices[i];
        let b = vertices[(i + 1) % vertices.len()];
        s += a.x * b.y - b.x * a.y;
    }
    0.5 * s
}

fn canonical_loop_key(vertices: &[Vec3]) -> String {
    fn min_rotation(seq: &[(i64, i64)]) -> Vec<(i64, i64)> {
        let n = seq.len();
        if n == 0 {
            return Vec::new();
        }
        let mut best = (0..n)
            .map(|i| {
                (0..n)
                    .map(|k| seq[(i + k) % n])
                    .collect::<Vec<(i64, i64)>>()
            })
            .next()
            .unwrap_or_default();
        for i in 1..n {
            let cand = (0..n)
                .map(|k| seq[(i + k) % n])
                .collect::<Vec<(i64, i64)>>();
            if cand < best {
                best = cand;
            }
        }
        best
    }

    let quant: Vec<(i64, i64)> = vertices
        .iter()
        .map(|p| ((p.x * 1_000_000.0).round() as i64, (p.y * 1_000_000.0).round() as i64))
        .collect();
    if quant.is_empty() {
        return String::new();
    }
    let fwd = min_rotation(&quant);
    let mut rev_seq = quant.clone();
    rev_seq.reverse();
    let rev = min_rotation(&rev_seq);
    let best = if rev < fwd { rev } else { fwd };
    best.iter()
        .map(|(x, y)| format!("{x}:{y}"))
        .collect::<Vec<String>>()
        .join("|")
}

/// Convenience overload that decomposes polylines into line segments first.
pub fn detect_closed_boundaries_from_polylines(polylines: &[Polyline], tol: f64) -> Vec<Polyline> {
    let mut segs: Vec<Line> = Vec::new();
    for p in polylines {
        segs.extend(p.segments());
    }
    detect_closed_boundaries(&segs, tol)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }

    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line::new(v(x0, y0), v(x1, y1))
    }

    #[test]
    fn finds_single_square() {
        let segs = vec![
            line(0.0, 0.0, 10.0, 0.0),
            line(10.0, 0.0, 10.0, 10.0),
            line(10.0, 10.0, 0.0, 10.0),
            line(0.0, 10.0, 0.0, 0.0),
        ];
        let loops = detect_closed_boundaries(&segs, 1e-6);
        assert_eq!(loops.len(), 1);
        assert!(loops[0].closed);
        assert_eq!(loops[0].vertices.len(), 4);
    }

    #[test]
    fn finds_two_disjoint_squares() {
        let segs = vec![
            line(0.0, 0.0, 1.0, 0.0),
            line(1.0, 0.0, 1.0, 1.0),
            line(1.0, 1.0, 0.0, 1.0),
            line(0.0, 1.0, 0.0, 0.0),
            line(3.0, 0.0, 4.0, 0.0),
            line(4.0, 0.0, 4.0, 1.0),
            line(4.0, 1.0, 3.0, 1.0),
            line(3.0, 1.0, 3.0, 0.0),
        ];
        let loops = detect_closed_boundaries(&segs, 1e-6);
        assert_eq!(loops.len(), 2);
    }

    #[test]
    fn open_chain_has_no_loop() {
        let segs = vec![
            line(0.0, 0.0, 2.0, 0.0),
            line(2.0, 0.0, 2.0, 2.0),
            line(2.0, 2.0, 0.5, 2.0),
        ];
        let loops = detect_closed_boundaries(&segs, 1e-6);
        assert!(loops.is_empty());
    }

    #[test]
    fn closed_polyline_detects() {
        let p = Polyline::new(vec![v(0.0, 0.0), v(2.0, 0.0), v(1.0, 1.0)], true);
        let loops = detect_closed_boundaries_from_polylines(&[p], 1e-6);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].vertices.len(), 3);
        assert!(loops[0].closed);
    }

    #[test]
    fn splits_at_mid_edge_t_junctions() {
        // Divider touches outer vertical edges at midpoints, not at original edge endpoints.
        // A valid detector must split those edges and find top + bottom rooms.
        let segs = vec![
            line(0.0, 0.0, 10.0, 0.0),
            line(10.0, 0.0, 10.0, 10.0),
            line(10.0, 10.0, 0.0, 10.0),
            line(0.0, 10.0, 0.0, 0.0),
            line(0.0, 5.0, 10.0, 5.0),
        ];
        let loops = detect_closed_boundaries(&segs, 1e-6);
        assert_eq!(loops.len(), 2);
    }
}
