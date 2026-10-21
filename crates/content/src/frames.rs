//! Comparing two views, square by square.
//!
//! The frame dumps come from the oracle (`--dump-frame`, see `oracle/harness/harness.cpp`) and, once the port
//! can generate the same map, from the port. This module is the comparison both sides meet in: it reads a dump
//! and reports the squares that differ, in a form that says what the reference saw and what the port saw.
//!
//! A diff that only says "different" is not worth running. The point of this one is that its output can be
//! read by a person looking at a screen, so it names the square, the field, and both values.

use cdda_json::{Object, Value};

/// One square of a frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSquare {
    /// Where it is.
    pub x: i32,
    /// Where it is.
    pub y: i32,
    /// What the interface would draw.
    pub glyph: String,
    /// The terrain's id.
    pub terrain: String,
    /// How many items lie there.
    pub items: i32,
    /// Ambient light.
    pub light: f64,
}

/// A whole view.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// How far the view reaches from its centre.
    pub radius: i32,
    /// Where it is centred.
    pub centre: (i32, i32),
    /// Every square, in the order the dump lists them.
    pub squares: Vec<FrameSquare>,
}

/// One square where two frames disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Where it is.
    pub x: i32,
    /// Where it is.
    pub y: i32,
    /// Which field.
    pub field: &'static str,
    /// What the first frame says.
    pub left: String,
    /// What the second says.
    pub right: String,
}

/// Reads a frame dump.
///
/// The dump is written by the oracle and by the port; both use integers for coordinates and a float for
/// light, and the conversions below are the exact cases those two shapes need.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub fn read(value: &Value) -> Option<Frame> {
    let Value::Object(root) = value else {
        return None;
    };
    let number = |object: &Object, key: &str| -> Option<f64> {
        match object.get(key) {
            Some(Value::Int(value)) => Some(*value as f64),
            Some(Value::Float(value)) => Some(*value),
            _ => None,
        }
    };
    let radius = i32::try_from(number(root, "radius")? as i64).ok()?;
    let Value::Array(centre) = root.get("centre")? else {
        return None;
    };
    let centre = (
        i32::try_from(*match centre.first()? {
            Value::Int(value) => value,
            _ => return None,
        })
        .ok()?,
        i32::try_from(*match centre.get(1)? {
            Value::Int(value) => value,
            _ => return None,
        })
        .ok()?,
    );
    let Value::Array(entries) = root.get("squares")? else {
        return None;
    };
    let mut squares = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(square) = entry else {
            return None;
        };
        let text = |key: &str| match square.get(key) {
            Some(Value::Str(value)) => Some(value.clone()),
            _ => None,
        };
        squares.push(FrameSquare {
            x: i32::try_from(number(square, "x")? as i64).ok()?,
            y: i32::try_from(number(square, "y")? as i64).ok()?,
            glyph: text("glyph")?,
            terrain: text("ter")?,
            items: i32::try_from(number(square, "items").unwrap_or(0.0) as i64).unwrap_or(0),
            light: number(square, "light").unwrap_or(0.0),
        });
    }
    Some(Frame {
        radius,
        centre,
        squares,
    })
}

/// Indexes a frame by square, so two frames can be matched regardless of the order they were written in.
fn by_square(frame: &Frame) -> std::collections::BTreeMap<(i32, i32), &FrameSquare> {
    frame
        .squares
        .iter()
        .map(|square| ((square.x, square.y), square))
        .collect()
}

/// Every square where two frames disagree, in reading order.
///
/// Light is compared to a hundredth, because two renderers rounding the same value differently is not a
/// difference a player would ever see and would drown the real ones.
#[must_use]
pub fn diff(left: &Frame, right: &Frame) -> Vec<Difference> {
    let mut out = Vec::new();
    let left_squares = by_square(left);
    let right_squares = by_square(right);
    for ((x, y), a) in &left_squares {
        let Some(b) = right_squares.get(&(*x, *y)).copied() else {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "square",
                left: "present".to_owned(),
                right: "missing".to_owned(),
            });
            continue;
        };
        if a.glyph != b.glyph {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "glyph",
                left: a.glyph.clone(),
                right: b.glyph.clone(),
            });
        }
        if a.terrain != b.terrain {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "terrain",
                left: a.terrain.clone(),
                right: b.terrain.clone(),
            });
        }
        if a.items != b.items {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "items",
                left: a.items.to_string(),
                right: b.items.to_string(),
            });
        }
        if (a.light - b.light).abs() >= 0.01 {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "light",
                left: format!("{:.2}", a.light),
                right: format!("{:.2}", b.light),
            });
        }
    }
    for (x, y) in right_squares.keys() {
        if !left_squares.contains_key(&(*x, *y)) {
            out.push(Difference {
                x: *x,
                y: *y,
                field: "square",
                left: "missing".to_owned(),
                right: "present".to_owned(),
            });
        }
    }
    out.sort_by_key(|difference| (difference.y, difference.x, difference.field));
    out
}
