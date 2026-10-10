//! Tolerant XML reader modelled on `Sexy::XMLParser`.
//!
//! The original `levels.xml` is not well-formed XML: it has unquoted
//! attribute values (`vx=-1`, `powerfreq=2500`), duplicated attributes
//! (`repeat="40" ... repeat="50"`) and spaces around `=`.  PopCap's parser
//! accepts all of that (the last duplicate wins), so this one does too.

#[derive(Debug, Clone, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    /// Attribute lookup (case-insensitive, last occurrence wins).
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }

    pub fn attr_f32(&self, key: &str) -> Option<f32> {
        self.attr(key).and_then(parse_f32)
    }

    pub fn attr_i32(&self, key: &str) -> Option<i32> {
        self.attr(key).and_then(parse_i32)
    }

    pub fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

pub fn parse_f32(s: &str) -> Option<f32> {
    let s = s.trim();
    s.parse::<f32>().ok().or_else(|| {
        // "1.5f", "3." etc.
        let t: String = s.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')).collect();
        t.parse().ok()
    })
}

pub fn parse_i32(s: &str) -> Option<i32> {
    let s = s.trim();
    s.parse::<i32>().ok().or_else(|| parse_f32(s).map(|f| f as i32))
}

/// Parses a document and returns a synthetic root whose children are the
/// top-level elements.
pub fn parse(text: &str) -> Element {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    let mut root = Element { name: "#document".into(), ..Default::default() };
    p.parse_children(&mut root);
    root
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat.as_bytes())
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    fn skip_until(&mut self, pat: &str) {
        while self.i < self.s.len() && !self.starts_with(pat) {
            self.i += 1;
        }
        self.i = (self.i + pat.len()).min(self.s.len());
    }

    fn read_name(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() || matches!(c, b'=' | b'>' | b'/' | b'<') {
                break;
            }
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }

    /// Parses children until the matching close tag (or EOF).
    fn parse_children(&mut self, parent: &mut Element) {
        loop {
            // Skip text content.
            while matches!(self.peek(), Some(c) if c != b'<') {
                self.i += 1;
            }
            if self.peek().is_none() {
                return;
            }
            if self.starts_with("<!--") {
                self.skip_until("-->");
            } else if self.starts_with("<?") || self.starts_with("<!") {
                self.skip_until(">");
            } else if self.starts_with("</") {
                self.skip_until(">");
                return;
            } else {
                self.i += 1; // '<'
                let mut el = Element { name: self.read_name(), ..Default::default() };
                let self_closing = self.parse_attrs(&mut el);
                if !self_closing {
                    self.parse_children(&mut el);
                }
                parent.children.push(el);
            }
        }
    }

    /// Returns true when the tag was self-closing (`/>`).
    fn parse_attrs(&mut self, el: &mut Element) -> bool {
        loop {
            self.skip_ws();
            match self.peek() {
                None => return true,
                Some(b'>') => {
                    self.i += 1;
                    return false;
                }
                Some(b'/') => {
                    self.i += 1;
                    self.skip_ws();
                    if self.peek() == Some(b'>') {
                        self.i += 1;
                    }
                    return true;
                }
                _ => {}
            }
            let key = self.read_name();
            if key.is_empty() {
                self.i += 1;
                continue;
            }
            self.skip_ws();
            let mut value = String::new();
            if self.peek() == Some(b'=') {
                self.i += 1;
                self.skip_ws();
                value = self.read_value();
            }
            el.attrs.push((key, value));
        }
    }

    fn read_value(&mut self) -> String {
        match self.peek() {
            Some(q @ (b'"' | b'\'')) => {
                self.i += 1;
                let start = self.i;
                while matches!(self.peek(), Some(c) if c != q) {
                    self.i += 1;
                }
                let v = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                self.i = (self.i + 1).min(self.s.len());
                v
            }
            _ => {
                let start = self.i;
                while let Some(c) = self.peek() {
                    if c.is_ascii_whitespace() || c == b'>' {
                        break;
                    }
                    // `vy=-1/>`: a slash directly before '>' ends the tag.
                    if c == b'/' && self.s.get(self.i + 1) == Some(&b'>') {
                        break;
                    }
                    self.i += 1;
                }
                String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_popcap_quirks() {
        let doc = parse(
            r#"<?xml version="1.0"?>
            <Levels>
            <!-- comment <Level/> -->
            <BackgroundAlpha image="quake02" x="293" y="0" vx=1 vy=-1/>
            <Settings id="level1" repeat="40" colors="4" repeat="50" />
            <StageProgression stage1 = "spiral,claw" diffi1 = "lvl11,lvl12" />
            <Graphics id="g"><Cutout image="t" pri="1" /></Graphics>
            </Levels>"#,
        );
        let levels = &doc.children[0];
        assert!(levels.is("levels"));
        assert_eq!(levels.children.len(), 4);
        let bg = &levels.children[0];
        assert_eq!(bg.attr_i32("vx"), Some(1));
        assert_eq!(bg.attr_i32("vy"), Some(-1));
        assert_eq!(levels.children[1].attr("repeat"), Some("50"));
        assert_eq!(levels.children[2].attr("diffi1"), Some("lvl11,lvl12"));
        assert_eq!(levels.children[3].children[0].attr("image"), Some("t"));
    }
}
