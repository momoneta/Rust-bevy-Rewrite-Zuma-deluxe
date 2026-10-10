//! PopCap `ImageFont` descriptor files (`fonts/*.txt`).
//!
//! The descriptor is a tiny script interpreted by `Sexy::DescParser` /
//! `Sexy::FontData`:
//!
//! ```text
//! Define CharList ('A', 'B', ...);
//! CreateLayer Main;
//! LayerSetImage Main 'Cancun12';
//! LayerSetImageMap Main CharList RectList;
//! ...
//! ```
//!
//! Only the commands used by Zuma's fonts are implemented.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Id(String),
    List(Vec<Value>),
}

impl Value {
    fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Int(i) => Some(*i as f32),
            Value::Float(f) => Some(*f as f32),
            _ => None,
        }
    }
    fn as_i32(&self) -> Option<i32> {
        self.as_f32().map(|f| f as i32)
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Id(s) => Some(s),
            _ => None,
        }
    }
    fn as_list(&self) -> Vec<Value> {
        match self {
            Value::List(l) => l.clone(),
            other => vec![other.clone()],
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Glyph {
    /// Source rectangle in the layer image: x, y, w, h.
    pub rect: [i32; 4],
    pub offset: (i32, i32),
    pub width: i32,
}

#[derive(Debug, Clone)]
pub struct FontLayer {
    pub name: String,
    pub image: String,
    pub ascent: i32,
    pub ascent_padding: i32,
    pub line_spacing_offset: i32,
    pub point_size: i32,
    pub offset: (i32, i32),
    pub color_mult: [f32; 4],
    pub color_add: [f32; 4],
    pub base_order: i32,
    pub required_tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    pub glyphs: HashMap<char, Glyph>,
    pub kerning: HashMap<(char, char), i32>,
}

impl FontLayer {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            image: String::new(),
            ascent: 0,
            ascent_padding: 0,
            line_spacing_offset: 0,
            point_size: 0,
            offset: (0, 0),
            color_mult: [1.0; 4],
            color_add: [0.0; 4],
            base_order: 0,
            required_tags: Vec::new(),
            excluded_tags: Vec::new(),
            glyphs: HashMap::new(),
            kerning: HashMap::new(),
        }
    }

    fn glyph_mut(&mut self, c: char) -> &mut Glyph {
        self.glyphs.entry(c).or_default()
    }

    /// Whether this layer is drawn for the given set of active tags.
    pub fn active(&self, tags: &[&str]) -> bool {
        self.required_tags.iter().all(|t| tags.iter().any(|a| a.eq_ignore_ascii_case(t)))
            && !self.excluded_tags.iter().any(|t| tags.iter().any(|a| a.eq_ignore_ascii_case(t)))
    }
}

#[derive(Debug, Clone, Default)]
pub struct FontDesc {
    pub layers: Vec<FontLayer>,
    pub char_map: HashMap<char, char>,
    pub default_point_size: i32,
}

impl FontDesc {
    pub fn map_char(&self, c: char) -> char {
        *self.char_map.get(&c).unwrap_or(&c)
    }

    /// The layer used for metrics: the first one without required tags.
    pub fn main_layer(&self) -> Option<&FontLayer> {
        self.layers.iter().find(|l| l.required_tags.is_empty()).or(self.layers.first())
    }

    pub fn ascent(&self) -> i32 {
        self.main_layer().map(|l| l.ascent).unwrap_or(0)
    }

    pub fn height(&self) -> i32 {
        self.main_layer().map(|l| l.glyphs.values().map(|g| g.rect[3]).max().unwrap_or(l.ascent).max(l.ascent)).unwrap_or(0)
    }

    pub fn line_spacing(&self) -> i32 {
        self.height() + self.main_layer().map(|l| l.line_spacing_offset).unwrap_or(0)
    }

    pub fn char_width(&self, c: char, next: Option<char>) -> i32 {
        let Some(l) = self.main_layer() else { return 0 };
        let c = self.map_char(c);
        let w = l.glyphs.get(&c).map(|g| g.width).unwrap_or(0);
        let k = next.map(|n| *l.kerning.get(&(c, self.map_char(n))).unwrap_or(&0)).unwrap_or(0);
        w + k
    }

    pub fn string_width(&self, s: &str) -> i32 {
        let chars: Vec<char> = s.chars().collect();
        (0..chars.len()).map(|i| self.char_width(chars[i], chars.get(i + 1).copied())).sum()
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let stmts = tokenize_statements(text)?;
        let mut defines: HashMap<String, Value> = HashMap::new();
        let mut font = FontDesc::default();

        // Resolve identifiers that name a Define.
        let resolve = |v: &Value, defines: &HashMap<String, Value>| -> Value {
            match v {
                Value::Id(id) => defines.get(&id.to_ascii_lowercase()).cloned().unwrap_or_else(|| v.clone()),
                other => other.clone(),
            }
        };

        for (cmd, args) in stmts {
            let cmd_l = cmd.to_ascii_lowercase();
            if cmd_l == "define" {
                if let (Some(Value::Id(name)), Some(v)) = (args.first(), args.get(1)) {
                    let v = resolve(v, &defines);
                    defines.insert(name.to_ascii_lowercase(), v);
                }
                continue;
            }
            let a: Vec<Value> = args.iter().map(|v| resolve(v, &defines)).collect();
            let layer_name = args.first().and_then(|v| match v {
                Value::Id(s) => Some(s.clone()),
                _ => None,
            });
            match cmd_l.as_str() {
                "createlayer" => {
                    if let Some(n) = &layer_name {
                        font.layers.push(FontLayer::new(n));
                    }
                }
                "setdefaultpointsize" => {
                    font.default_point_size = a.first().and_then(Value::as_i32).unwrap_or(0);
                }
                "setcharmap" => {
                    let from = a.first().map(Value::as_list).unwrap_or_default();
                    let to = a.get(1).map(Value::as_list).unwrap_or_default();
                    for (f, t) in from.iter().zip(to.iter()) {
                        if let (Some(f), Some(t)) = (to_char(f), to_char(t)) {
                            font.char_map.insert(f, t);
                        }
                    }
                }
                _ if cmd_l.starts_with("layer") => {
                    let Some(name) = layer_name else { continue };
                    let Some(layer) = font.layers.iter_mut().find(|l| l.name.eq_ignore_ascii_case(&name)) else {
                        continue;
                    };
                    apply_layer_cmd(layer, &cmd_l, &a[1..]);
                }
                _ => {}
            }
        }
        if font.layers.is_empty() {
            return Err("font has no layers".into());
        }
        font.layers.sort_by_key(|l| l.base_order);
        Ok(font)
    }
}

fn to_char(v: &Value) -> Option<char> {
    match v {
        Value::Str(s) => s.chars().next(),
        Value::Int(i) => char::from_u32(*i as u32),
        _ => None,
    }
}

fn pair(v: &Value) -> (i32, i32) {
    let l = v.as_list();
    (l.first().and_then(Value::as_i32).unwrap_or(0), l.get(1).and_then(Value::as_i32).unwrap_or(0))
}

fn color(v: &Value) -> [f32; 4] {
    let l = v.as_list();
    let g = |i: usize, d: f32| l.get(i).and_then(Value::as_f32).unwrap_or(d);
    [g(0, 1.0), g(1, 1.0), g(2, 1.0), g(3, 1.0)]
}

fn apply_layer_cmd(layer: &mut FontLayer, cmd: &str, a: &[Value]) {
    let first_i = || a.first().and_then(Value::as_i32).unwrap_or(0);
    match cmd {
        "layersetimage" => layer.image = a.first().and_then(Value::as_str).unwrap_or_default().to_string(),
        "layersetascent" => layer.ascent = first_i(),
        "layersetascentpadding" => layer.ascent_padding = first_i(),
        "layersetlinespacingoffset" => layer.line_spacing_offset = first_i(),
        "layersetpointsize" => layer.point_size = first_i(),
        "layersetbaseorder" => layer.base_order = first_i(),
        "layersetoffset" => layer.offset = a.first().map(pair).unwrap_or((0, 0)),
        "layersetcolormult" => layer.color_mult = a.first().map(color).unwrap_or([1.0; 4]),
        "layersetcoloradd" => layer.color_add = a.first().map(color).unwrap_or([0.0; 4]),
        "layerrequiretags" | "layerexcludetags" => {
            let tags: Vec<String> =
                a.first().map(Value::as_list).unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
            if cmd == "layerrequiretags" {
                layer.required_tags = tags;
            } else {
                layer.excluded_tags = tags;
            }
        }
        "layersetcharwidths" | "layersetimagemap" | "layersetcharoffsets" => {
            let chars = a.first().map(Value::as_list).unwrap_or_default();
            let vals = a.get(1).map(Value::as_list).unwrap_or_default();
            for (c, v) in chars.iter().zip(vals.iter()) {
                let Some(c) = to_char(c) else { continue };
                let g = layer.glyph_mut(c);
                match cmd {
                    "layersetcharwidths" => g.width = v.as_i32().unwrap_or(0),
                    "layersetimagemap" => {
                        let l = v.as_list();
                        let n = |i: usize| l.get(i).and_then(Value::as_i32).unwrap_or(0);
                        g.rect = [n(0), n(1), n(2), n(3)];
                    }
                    _ => g.offset = pair(v),
                }
            }
        }
        "layersetkerningpairs" => {
            let pairs = a.first().map(Value::as_list).unwrap_or_default();
            let vals = a.get(1).map(Value::as_list).unwrap_or_default();
            for (p, v) in pairs.iter().zip(vals.iter()) {
                let Some(s) = p.as_str() else { continue };
                let mut it = s.chars();
                if let (Some(c1), Some(c2)) = (it.next(), it.next()) {
                    layer.kerning.insert((c1, c2), v.as_i32().unwrap_or(0));
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tokenizer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Value(Value),
    Open,
    Close,
    Comma,
    Semi,
}

fn lex(text: &str) -> Result<Vec<Tok>, String> {
    let s: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < s.len() {
        let c = s[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && s.get(i + 1) == Some(&'/') {
            while i < s.len() && s[i] != '\n' {
                i += 1;
            }
        } else if c == '(' {
            out.push(Tok::Open);
            i += 1;
        } else if c == ')' {
            out.push(Tok::Close);
            i += 1;
        } else if c == ',' {
            out.push(Tok::Comma);
            i += 1;
        } else if c == ';' {
            out.push(Tok::Semi);
            i += 1;
        } else if c == '\'' || c == '"' {
            let q = c;
            i += 1;
            let mut v = String::new();
            while i < s.len() && s[i] != q {
                if s[i] == '\\' && i + 1 < s.len() {
                    i += 1;
                }
                v.push(s[i]);
                i += 1;
            }
            i += 1;
            out.push(Tok::Value(Value::Str(v)));
        } else if c.is_ascii_digit() || ((c == '-' || c == '.') && s.get(i + 1).is_some_and(|n| n.is_ascii_digit() || *n == '.')) {
            let start = i;
            i += 1;
            while i < s.len() && (s[i].is_ascii_digit() || s[i] == '.') {
                i += 1;
            }
            let t: String = s[start..i].iter().collect();
            if t.contains('.') {
                out.push(Tok::Value(Value::Float(t.parse().map_err(|_| format!("bad number {t}"))?)));
            } else {
                out.push(Tok::Value(Value::Int(t.parse().map_err(|_| format!("bad number {t}"))?)));
            }
        } else {
            let start = i;
            while i < s.len() && (s[i].is_alphanumeric() || s[i] == '_') {
                i += 1;
            }
            if i == start {
                return Err(format!("unexpected character {c:?}"));
            }
            out.push(Tok::Value(Value::Id(s[start..i].iter().collect())));
        }
    }
    Ok(out)
}

fn parse_value(toks: &[Tok], i: &mut usize) -> Option<Value> {
    match toks.get(*i)? {
        Tok::Value(v) => {
            *i += 1;
            Some(v.clone())
        }
        Tok::Open => {
            *i += 1;
            let mut items = Vec::new();
            loop {
                match toks.get(*i) {
                    None => break,
                    Some(Tok::Close) => {
                        *i += 1;
                        break;
                    }
                    Some(Tok::Comma) => *i += 1,
                    Some(_) => items.push(parse_value(toks, i)?),
                }
            }
            Some(Value::List(items))
        }
        _ => None,
    }
}

fn tokenize_statements(text: &str) -> Result<Vec<(String, Vec<Value>)>, String> {
    let toks = lex(text)?;
    let mut i = 0;
    let mut out = Vec::new();
    while i < toks.len() {
        let cmd = match &toks[i] {
            Tok::Value(Value::Id(id)) => id.clone(),
            Tok::Semi => {
                i += 1;
                continue;
            }
            t => return Err(format!("expected command, got {t:?}")),
        };
        i += 1;
        let mut args = Vec::new();
        while i < toks.len() && toks[i] != Tok::Semi {
            match parse_value(&toks, &mut i) {
                Some(v) => args.push(v),
                None => i += 1,
            }
        }
        i += 1;
        out.push((cmd, args));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_descriptor() {
        let f = FontDesc::parse(
            r#"Define CharList ('A', "'", '"', 'B');
            Define WidthList (10, 3, 4, 11);
            Define RectList ((0,0,9,13), (10,0,2,13), (13,0,3,13), (17,0,10,13));
            Define KerningPairs ("\"A", "AB");
            Define KerningValues (-2, -1);
            CreateLayer Main;
            LayerSetImage Main 'Cancun12';
            LayerSetAscent Main 13;
            LayerSetCharWidths Main CharList WidthList;
            LayerSetCharWidths Main (' ') (4);
            LayerSetImageMap Main CharList RectList;
            LayerSetKerningPairs Main KerningPairs KerningValues;
            CreateLayer Shadow;
            LayerSetColorMult Shadow (0, 0, 0, 0.75);
            LayerSetOffset Shadow (1, -1);
            LayerSetBaseOrder Shadow -1;
            LayerRequireTags Shadow ("Shadow");
            SetCharMap ('a') ('A');
            SetDefaultPointSize 12;"#,
        )
        .unwrap();
        assert_eq!(f.layers.len(), 2);
        assert_eq!(f.layers[0].name, "Shadow"); // sorted by base order
        let main = f.main_layer().unwrap();
        assert_eq!(main.image, "Cancun12");
        assert_eq!(main.glyphs[&'B'].rect, [17, 0, 10, 13]);
        assert_eq!(main.kerning[&('"', 'A')], -2);
        assert_eq!(f.string_width("AB"), 10 - 1 + 11);
        assert_eq!(f.string_width("a "), 10 + 4);
        assert_eq!(f.layers[0].color_mult[3], 0.75);
        assert_eq!(f.layers[0].offset, (1, -1));
    }
}
