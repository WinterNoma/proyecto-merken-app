//! Prevalidación conservadora de la sintaxis GBNF antes del parser nativo.
use std::collections::{HashMap, HashSet};

const MAX_GRAMMAR: usize = 256 * 1024;
const MAX_DEPTH: usize = 32;

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    refs: HashSet<String>,
    definitions: HashSet<String>,
    complexity: usize,
    current: String,
    edges: HashMap<String, HashSet<String>>,
    _source: &'a str,
}
impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }
    fn take(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn space(&mut self, lines: bool) {
        loop {
            match self.peek() {
                Some(' ' | '\t') => {
                    let _ = self.next();
                }
                Some('\r' | '\n') if lines => {
                    let _ = self.next();
                }
                Some('#') => {
                    while !matches!(self.peek(), None | Some('\r' | '\n')) {
                        let _ = self.next();
                    }
                }
                _ => break,
            }
        }
    }
    fn name(&mut self) -> Option<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '-' {
                name.push(c);
                let _ = self.next();
            } else {
                break;
            }
        }
        if name.is_empty() { None } else { Some(name) }
    }
    fn character(&mut self) -> Result<(), ()> {
        match self.next() {
            Some('\\') => match self.next() {
                Some('t' | 'r' | 'n' | '\\' | '"' | '[' | ']' | '-') => Ok(()),
                Some(c @ ('x' | 'u' | 'U')) => {
                    let n = match c {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    for _ in 0..n {
                        if !self.next().is_some_and(|v| v.is_ascii_hexdigit()) {
                            return Err(());
                        }
                    }
                    Ok(())
                }
                _ => Err(()),
            },
            Some(c) if !c.is_control() => Ok(()),
            _ => Err(()),
        }
    }
    fn number(&mut self) -> Result<u32, ()> {
        let mut n = 0_u32;
        let mut found = false;
        while let Some(c) = self
            .peek()
            .filter(char::is_ascii_digit)
            .and_then(|c| c.to_digit(10))
        {
            found = true;
            let _ = self.next();
            n = n.checked_mul(10).and_then(|v| v.checked_add(c)).ok_or(())?;
            if n > 32 {
                return Err(());
            }
        }
        if found { Ok(n) } else { Err(()) }
    }
    fn expression(&mut self, depth: usize) -> Result<bool, ()> {
        if depth > MAX_DEPTH || self.complexity > 1000 {
            return Err(());
        }
        let mut all_nonempty = true;
        loop {
            let mut items = 0;
            let mut sequence_nonempty = false;
            loop {
                self.space(depth > 0);
                let mut empty = false;
                let mut nonempty = false;
                let group = match self.peek() {
                    Some('"') => {
                        let _ = self.next();
                        empty = self.peek() == Some('"');
                        nonempty = !empty;
                        while self.peek() != Some('"') {
                            self.character()?;
                        }
                        let _ = self.next();
                        false
                    }
                    Some('[') => {
                        let _ = self.next();
                        self.take('^');
                        let mut count = 0;
                        while self.peek() != Some(']') {
                            self.character()?;
                            count += 1;
                            if self.peek() == Some('-')
                                && self.chars.get(self.pos + 1).copied() != Some(']')
                            {
                                let _ = self.next();
                                self.character()?;
                            }
                        }
                        if count == 0 {
                            return Err(());
                        }
                        nonempty = true;
                        let _ = self.next();
                        false
                    }
                    Some('(') => {
                        let _ = self.next();
                        self.complexity += 1;
                        nonempty = self.expression(depth + 1)?;
                        self.space(true);
                        if !self.take(')') {
                            return Err(());
                        }
                        true
                    }
                    Some(c) if c.is_ascii_alphanumeric() || c == '-' => {
                        let name = self.name().ok_or(())?;
                        self.refs.insert(name.clone());
                        self.edges
                            .entry(self.current.clone())
                            .or_default()
                            .insert(name);
                        false
                    }
                    _ => break,
                };
                items += 1;
                self.space(depth > 0);
                match self.peek() {
                    Some(op @ ('*' | '+' | '?')) => {
                        if empty || (op != '?' && !nonempty) {
                            return Err(());
                        }
                        if op != '+' {
                            nonempty = false;
                        }
                        self.complexity += 1;
                        let _ = self.next();
                    }
                    Some('{') => {
                        // Evita la expansión multiplicativa nativa de grupos repetidos.
                        if group || empty || !nonempty {
                            return Err(());
                        }
                        self.complexity += 32;
                        let _ = self.next();
                        self.space(depth > 0);
                        let min = self.number()?;
                        if min == 0 {
                            nonempty = false;
                        }
                        self.space(depth > 0);
                        if self.take(',') {
                            self.space(depth > 0);
                            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                                let max = self.number()?;
                                if max < min {
                                    return Err(());
                                }
                            }
                            self.space(depth > 0);
                        }
                        if !self.take('}') {
                            return Err(());
                        }
                    }
                    _ => {}
                }
                sequence_nonempty |= nonempty;
            }
            if items == 0 {
                return Err(());
            }
            self.space(depth > 0);
            all_nonempty &= sequence_nonempty;
            if !self.take('|') {
                return Ok(all_nonempty);
            }
            self.space(true);
        }
    }
}
pub(crate) fn validate(source: &str) -> Result<(), ()> {
    if source.len() > MAX_GRAMMAR || source.contains('\0') {
        return Err(());
    }
    let mut p = Parser {
        chars: source.chars().collect(),
        pos: 0,
        refs: HashSet::new(),
        definitions: HashSet::new(),
        complexity: 0,
        current: String::new(),
        edges: HashMap::new(),
        _source: source,
    };
    p.space(true);
    while p.peek().is_some() {
        let name = p.name().ok_or(())?;
        if !p.definitions.insert(name.clone()) || p.definitions.len() > 512 {
            return Err(());
        }
        p.current = name;
        p.space(false);
        if !(p.take(':') && p.take(':') && p.take('=')) {
            return Err(());
        }
        p.space(true);
        p.expression(0)?;
        p.space(false);
        if !matches!(p.peek(), None | Some('\n' | '\r')) {
            return Err(());
        }
        p.space(true);
    }
    if p.complexity > 1000 || !p.definitions.contains("root") || !p.refs.is_subset(&p.definitions) {
        return Err(());
    }
    // Rechaza ciclos, incluso los que no son recursión izquierda: la inicialización
    // nativa no admite recursión izquierda y este filtro evita analizar nulabilidad.
    let mut remaining = p.definitions;
    while !remaining.is_empty() {
        let ready: Vec<String> = remaining
            .iter()
            .filter(|name| {
                p.edges
                    .get(*name)
                    .is_none_or(|deps| deps.is_disjoint(&remaining))
            })
            .cloned()
            .collect();
        if ready.is_empty() {
            return Err(());
        }
        for name in ready {
            remaining.remove(&name);
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::validate;
    #[test]
    fn production_grammars() {
        assert!(validate(crate::PROPOSAL_GBNF).is_ok());
        for ids in [&[][..], &["one", "two"][..], &["a\"b", "a\\b", "a/b"][..]] {
            assert!(validate(&crate::build_selection_grammar(ids)).is_ok());
        }
    }
    #[test]
    fn rejects_invalid_input() {
        for grammar in [
            "root ::= missing",
            "root ::= root",
            r#"root ::= ("")*"#,
            "root ::= \"x",
            "root ::= [a",
            "other ::= \"a\"",
            "root ::= \"a\"{1,2",
            "root ::= \"a\" @",
            "root ::= \"a\"{3,2}",
            "root ::= \"a\"\\z",
        ] {
            assert!(validate(grammar).is_err());
        }
    }
}
