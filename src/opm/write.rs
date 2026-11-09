//! OP2MissionEditor's text layout (its `Utility.JsonFormatter` over `DataContractJsonSerializer` output), as its
//! files show it: a line break first, tab indents, CRLF line ends, members in alphabetical (ordinal) order, an object
//! or array value on the line after its member name, a blank indented line before each object in an array and inside
//! an empty array, and no line break at the end.

use serde_json::Value;

pub fn editor_style(v: &Value) -> String {
    let mut out = String::from("\r\n");
    value(v, 0, &mut out);
    out
}

fn indent(out: &mut String, depth: usize) {
    out.extend(std::iter::repeat_n('\t', depth));
}

fn value(v: &Value, depth: usize, out: &mut String) {
    match v {
        Value::Object(map) => {
            indent(out, depth);
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, key) in keys.into_iter().enumerate() {
                out.push_str(if i == 0 { "\r\n" } else { ",\r\n" });
                indent(out, depth + 1);
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                let member = &map[key];
                if member.is_object() || member.is_array() {
                    out.push_str("\r\n");
                    value(member, depth + 1, out);
                } else {
                    scalar(member, out);
                }
            }
            out.push_str("\r\n");
            indent(out, depth);
            out.push('}');
        }
        Value::Array(items) => {
            indent(out, depth);
            out.push_str("[\r\n");
            if items.is_empty() {
                indent(out, depth + 1);
                out.push_str("\r\n");
            }
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\r\n");
                }
                if item.is_object() || item.is_array() {
                    indent(out, depth + 1);
                    out.push_str("\r\n");
                    value(item, depth + 1, out);
                } else {
                    indent(out, depth + 1);
                    scalar(item, out);
                }
            }
            if !items.is_empty() {
                out.push_str("\r\n");
            }
            indent(out, depth);
            out.push(']');
        }
        _ => {
            indent(out, depth);
            scalar(v, out);
        }
    }
}

/// A number with no fraction is written without one (`"Health":1`), as .NET does.
fn scalar(v: &Value, out: &mut String) {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 1e15 => out.push_str(&(f as i64).to_string()),
            _ => out.push_str(&n.to_string()),
        },
        other => out.push_str(&other.to_string()),
    }
}
