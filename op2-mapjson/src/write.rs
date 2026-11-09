//! OP2MapJsonTools' PerRowPadded layout: Newtonsoft.Json's indented output (two spaces, `"key": value`, CRLF, no
//! line break at the end, each value of a number list on its own line), except that each row of `tiles` and
//! `cellTypes` and each set of wall tile mappings is one line, `[ a, b, c ]`, with its numbers right-aligned to the
//! widest of the row: at least 4 characters in `tiles` and 2 in `cellTypes`.

use serde_json::{Map as JsonMap, Value};

use crate::{MapJson, Range16, TerrainType};

const NL: &str = "\r\n";

/// Builds a JSON object member by member, at one indent level.
struct Object<'a> {
    out: &'a mut String,
    indent: usize,
    first: bool,
}

impl<'a> Object<'a> {
    fn open(out: &'a mut String, indent: usize) -> Self {
        out.push('{');
        Self { out, indent, first: true }
    }

    /// Starts member `key`; the caller writes its value.
    fn key(&mut self, key: &str) -> &mut String {
        self.out.push_str(if self.first { NL } else { "," });
        if !self.first {
            self.out.push_str(NL);
        }
        self.first = false;
        pad(self.out, self.indent + 1);
        self.out.push_str(&quote(key));
        self.out.push_str(": ");
        self.out
    }

    fn num(&mut self, key: &str, v: impl ToString) {
        let s = v.to_string();
        self.key(key).push_str(&s);
    }

    fn str(&mut self, key: &str, v: &str) {
        let s = quote(v);
        self.key(key).push_str(&s);
    }

    /// A list of numbers, one per line.
    fn list(&mut self, key: &str, values: impl IntoIterator<Item = i64>) {
        let indent = self.indent + 1;
        let out = self.key(key);
        let values: Vec<i64> = values.into_iter().collect();
        if values.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push('[');
        for (i, v) in values.iter().enumerate() {
            out.push_str(if i == 0 { NL } else { "," });
            if i > 0 {
                out.push_str(NL);
            }
            pad(out, indent + 1);
            out.push_str(&v.to_string());
        }
        out.push_str(NL);
        pad(out, indent);
        out.push(']');
    }

    /// A list of rows, each on one line with its numbers padded to `min_width` or the row's widest.
    fn rows<R: AsRef<[u16]>>(&mut self, key: &str, rows: &[R], min_width: usize) {
        let indent = self.indent + 1;
        let out = self.key(key);
        if rows.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push('[');
        for (i, row) in rows.iter().enumerate() {
            out.push_str(if i == 0 { NL } else { "," });
            if i > 0 {
                out.push_str(NL);
            }
            pad(out, indent + 1);
            let numbers: Vec<String> = row.as_ref().iter().map(u16::to_string).collect();
            let width = numbers.iter().map(String::len).max().unwrap_or(0).max(min_width);
            out.push_str("[ ");
            out.push_str(&numbers.iter().map(|n| format!("{n:>width$}")).collect::<Vec<_>>().join(", "));
            out.push_str(" ]");
        }
        out.push_str(NL);
        pad(out, indent);
        out.push(']');
    }

    /// A member whose value is another object.
    fn object(&mut self, key: &str, fill: impl FnOnce(&mut Object)) {
        let indent = self.indent + 1;
        let out = self.key(key);
        let mut inner = Object::open(out, indent);
        fill(&mut inner);
        inner.close();
    }

    /// A list of objects.
    fn objects<T>(&mut self, key: &str, items: &[T], fill: impl Fn(&mut Object, &T)) {
        let indent = self.indent + 1;
        let out = self.key(key);
        if items.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push('[');
        for (i, item) in items.iter().enumerate() {
            out.push_str(if i == 0 { NL } else { "," });
            if i > 0 {
                out.push_str(NL);
            }
            pad(out, indent + 1);
            let mut inner = Object::open(out, indent + 1);
            fill(&mut inner, item);
            inner.close();
        }
        out.push_str(NL);
        pad(out, indent);
        out.push(']');
    }

    /// Members this library doesn't know, written as Newtonsoft would.
    fn extra(&mut self, extra: &JsonMap<String, Value>) {
        for (k, v) in extra {
            let indent = self.indent + 1;
            let out = self.key(k);
            value(out, v, indent);
        }
    }

    fn close(self) {
        if self.first {
            self.out.push('}');
            return;
        }
        self.out.push_str(NL);
        pad(self.out, self.indent);
        self.out.push('}');
    }
}

fn pad(out: &mut String, indent: usize) {
    out.extend(std::iter::repeat_n(' ', indent * 2));
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("a string always converts")
}

/// Any JSON value in Newtonsoft's indented layout.
fn value(out: &mut String, v: &Value, indent: usize) {
    match v {
        Value::Object(map) => {
            let mut o = Object::open(out, indent);
            o.extra(map);
            o.close();
        }
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i == 0 { NL } else { "," });
                if i > 0 {
                    out.push_str(NL);
                }
                pad(out, indent + 1);
                value(out, item, indent + 1);
            }
            out.push_str(NL);
            pad(out, indent);
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

fn range(o: &mut Object, key: &str, r: &Range16) {
    o.object(key, |o| {
        o.num("start", r.start);
        o.num("end", r.end);
    });
}

fn terrain_type(o: &mut Object, t: &TerrainType) {
    range(o, "tileRange", &t.tile_range);
    o.num("bulldozedTileMappingIndex", t.bulldozed_tile_mapping_index);
    o.num("rubbleTileMappingIndex", t.rubble_tile_mapping_index);
    o.list("tubeTileMappings", t.tube_tile_mappings.map(i64::from));
    o.rows("wallTileMappingIndexes", &t.wall_tile_mapping_indexes, 1);
    o.num("lavaTileMappingIndex", t.lava_tile_mapping_index);
    o.num("flat1", t.flat1);
    o.num("flat2", t.flat2);
    o.num("flat3", t.flat3);
    o.list("tubeTileMappingIndexes", t.tube_tile_mapping_indexes.map(i64::from));
    o.num("scorchedTileMappingIndex", t.scorched_tile_mapping_index);
    o.objects("scorchedRange", &t.scorched_range, |o, r| {
        o.num("start", r.start);
        o.num("end", r.end);
    });
    o.list("unknown", t.unknown.map(i64::from));
}

pub fn per_row_padded(m: &MapJson) -> String {
    let mut out = String::new();
    let mut root = Object::open(&mut out, 0);
    root.object("header", |o| {
        let h = &m.header;
        o.num("width", h.width);
        o.num("height", h.height);
        o.str("map", &h.map);
        o.str("name", &h.name);
        o.str("author", &h.author);
        o.str("notes", &h.notes);
        o.extra(&h.extra);
    });
    root.rows("tiles", &m.tiles, 4);
    let cells: Vec<Vec<u16>> = m.cell_types.iter().map(|r| r.iter().map(|&c| c as u16).collect()).collect();
    root.rows("cellTypes", &cells, 2);
    if let Some(c) = &m.clip_rect {
        root.object("clipRect", |o| {
            o.num("x1", c.x1);
            o.num("y1", c.y1);
            o.num("x2", c.x2);
            o.num("y2", c.y2);
        });
    }
    root.object("tileset", |o| {
        let t = &m.tileset;
        o.objects("sources", &t.sources, |o, s| {
            o.str("filename", &s.filename);
            o.num("numTiles", s.num_tiles);
        });
        o.objects("tileMappings", &t.tile_mappings, |o, tm| {
            o.num("tilesetIndex", tm.tileset_index);
            o.num("tileGraphicIndex", tm.tile_graphic_index);
            o.num("animationCount", tm.animation_count);
            o.num("animationDelay", tm.animation_delay);
        });
        o.objects("terrainTypes", &t.terrain_types, terrain_type);
        o.extra(&t.extra);
    });
    root.extra(&m.extra);
    root.close();
    out
}
