//! Text normalization and lexicon coverage, independent of the native engine.
use crate::VoiceError;
use std::collections::HashSet;
#[cfg(any(feature = "tts", test))]
use std::io::Read;
#[cfg(any(feature = "tts", test))]
use std::path::Path;

#[cfg(any(feature = "tts", test))]
const MAX_LEXICON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXT: usize = 600;

pub(crate) struct Lexicon {
    words: HashSet<String>,
}
impl Lexicon {
    #[cfg(any(feature = "tts", test))]
    pub(crate) fn load(path: &Path) -> Result<Self, VoiceError> {
        let file = std::fs::File::open(path).map_err(|_| VoiceError::LoadFailed)?;
        let mut bytes = Vec::new();
        file.take(MAX_LEXICON_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| VoiceError::LoadFailed)?;
        if bytes.len() as u64 > MAX_LEXICON_BYTES {
            return Err(VoiceError::LoadFailed);
        }
        let contents = std::str::from_utf8(&bytes).map_err(|_| VoiceError::LoadFailed)?;
        Self::parse(contents)
    }
    #[cfg(any(feature = "tts", test))]
    pub(crate) fn load_many(paths: &[&Path]) -> Result<Self, VoiceError> {
        let mut words = HashSet::new();
        for path in paths {
            words.extend(Self::load(path)?.words);
        }
        if words.is_empty() {
            return Err(VoiceError::LoadFailed);
        }
        Ok(Self { words })
    }
    #[cfg(any(feature = "tts", test))]
    fn parse(contents: &str) -> Result<Self, VoiceError> {
        let mut words = HashSet::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Two formats: `word(s) || phones` (standalone lexicon) and
            // `word phones...` (lexicon bundled with Kokoro v1.0).
            let (word, pronunciation) = match line.split_once(" || ") {
                Some(pair) => pair,
                None => line
                    .split_once(char::is_whitespace)
                    .ok_or(VoiceError::LoadFailed)?,
            };
            let word = word.trim();
            if word.is_empty() || pronunciation.trim().is_empty() {
                return Err(VoiceError::LoadFailed);
            }
            // Multi-word entries do not make their words individually speakable.
            if word.contains(char::is_whitespace) {
                continue;
            }
            words.insert(word.to_lowercase());
        }
        if words.is_empty() {
            return Err(VoiceError::LoadFailed);
        }
        Ok(Self { words })
    }
    /// Returns prepared text and distinct OOV tokens, in their first occurrence order.
    pub(crate) fn check(&self, text: &str) -> (String, Vec<String>) {
        let mut prepared = Vec::new();
        let mut missing = Vec::new();
        for token in text.split_whitespace() {
            if token.chars().all(is_punctuation) || self.words.contains(&token.to_lowercase()) {
                prepared.push(token.to_owned());
            } else if token.len() <= 5
                && token.len() > 1
                && token.chars().all(|c| c.is_ascii_uppercase())
                && token
                    .chars()
                    .all(|c| self.words.contains(&c.to_ascii_lowercase().to_string()))
            {
                prepared.extend(token.chars().map(|c| c.to_string()));
            } else {
                if !missing
                    .iter()
                    .any(|word: &String| word.eq_ignore_ascii_case(token))
                {
                    missing.push(token.to_owned());
                }
                prepared.push(token.to_owned());
            }
        }
        (prepared.join(" "), missing)
    }
}

fn is_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '(' | ')' | '"')
}
fn unit(word: &str) -> Option<&'static str> {
    match word.to_ascii_lowercase().as_str() {
        "bpm" => Some("beats a minute"),
        "mg/dl" => Some("milligrams a deciliter"),
        "mmhg" => Some("millimeters of mercury"),
        "°c" | "ºc" => Some("degrees Celsius"),
        "%" => Some("percent"),
        "kg" => Some("kilograms"),
        "h" => Some("hours"),
        "min" => Some("minutes"),
        "ml" => Some("milliliters"),
        "spo2" => Some("oxygen saturation"),
        "hrv" => Some("heart rate variability"),
        _ => None,
    }
}
fn number(n: u32) -> String {
    const SMALL: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if n < 20 {
        return SMALL.get(n as usize).copied().unwrap_or("").to_owned();
    }
    if n < 100 {
        let ten = TENS.get((n / 10) as usize).copied().unwrap_or("");
        return if n.is_multiple_of(10) {
            ten.to_owned()
        } else {
            format!("{ten} {}", number(n % 10))
        };
    }
    if n < 1000 {
        return if n.is_multiple_of(100) {
            format!("{} hundred", number(n / 100))
        } else {
            format!("{} hundred {}", number(n / 100), number(n % 100))
        };
    }
    if n.is_multiple_of(1000) {
        format!("{} thousand", number(n / 1000))
    } else {
        format!("{} thousand {}", number(n / 1000), number(n % 1000))
    }
}
fn numeric(raw: &str) -> Result<String, VoiceError> {
    let (negative, raw) = match raw.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, raw),
    };
    let (whole, fraction) = match raw.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (raw, None),
    };
    if whole.contains(',') {
        let mut groups = whole.split(',');
        let first = groups.next().unwrap_or("");
        if first.is_empty()
            || first.len() > 3
            || !first.chars().all(|c| c.is_ascii_digit())
            || groups.any(|g| g.len() != 3 || !g.chars().all(|c| c.is_ascii_digit()))
        {
            return Err(VoiceError::TextInvalid);
        }
    }
    let digits = whole.replace(',', "");
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(VoiceError::TextInvalid);
    }
    let value: u32 = digits.parse().map_err(|_| VoiceError::TextInvalid)?;
    if value > 999_999 {
        return Err(VoiceError::TextInvalid);
    }
    let mut result = if negative {
        format!("minus {}", number(value))
    } else {
        number(value)
    };
    if let Some(fraction) = fraction {
        if fraction.is_empty() || !fraction.chars().all(|c| c.is_ascii_digit()) {
            return Err(VoiceError::TextInvalid);
        }
        result.push_str(" point");
        for c in fraction.chars() {
            let digit = c.to_digit(10).ok_or(VoiceError::TextInvalid)?;
            result.push(' ');
            result.push_str(&number(digit));
        }
    }
    Ok(result)
}
fn time(raw: &str) -> Result<String, VoiceError> {
    let (h, m) = raw.split_once(':').ok_or(VoiceError::TextInvalid)?;
    if h.is_empty()
        || h.len() > 2
        || m.len() != 2
        || !h.chars().chain(m.chars()).all(|c| c.is_ascii_digit())
    {
        return Err(VoiceError::TextInvalid);
    }
    let hour: u32 = h.parse().map_err(|_| VoiceError::TextInvalid)?;
    let minute: u32 = m.parse().map_err(|_| VoiceError::TextInvalid)?;
    if hour > 23 || minute > 59 {
        return Err(VoiceError::TextInvalid);
    }
    Ok(if minute == 0 {
        format!("{} hundred", number(hour))
    } else if minute < 10 {
        format!("{} oh {}", number(hour), number(minute))
    } else {
        format!("{} {}", number(hour), number(minute))
    })
}
pub(crate) fn normalize_text(text: &str) -> Result<String, VoiceError> {
    if text.chars().count() > MAX_TEXT {
        return Err(VoiceError::TextInvalid);
    }
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    let mut previous_number = false;
    while let Some(ch) = chars.next() {
        if ch.is_control() && ch != '\n' && ch != '\r' {
            return Err(VoiceError::TextInvalid);
        }
        if ch.is_whitespace() {
            continue;
        }
        if ch.is_ascii_digit() || (ch == '-' && chars.peek().is_some_and(|c| c.is_ascii_digit())) {
            let mut raw = String::from(ch);
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == ',' || c == ':' || c == '.' {
                    // A trailing full stop is punctuation, not a decimal point.
                    if c == '.' || c == ',' {
                        let mut look = chars.clone();
                        look.next();
                        if !look.peek().is_some_and(|next| next.is_ascii_digit()) {
                            break;
                        }
                    }
                    raw.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            previous_number = true;
            tokens.push(if raw.contains(':') {
                time(&raw)?
            } else {
                numeric(&raw)?
            });
        } else if ch.is_alphabetic() || ch == '°' || ch == 'º' {
            previous_number = false;
            let mut word = String::from(ch);
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '\'' | '°' | 'º'))
            {
                if let Some(c) = chars.next() {
                    word.push(c);
                }
            }
            if word.eq_ignore_ascii_case("mg") {
                let mut look = chars.clone();
                if look.next() == Some('/')
                    && look.next().is_some_and(|c| c.eq_ignore_ascii_case(&'d'))
                    && look.next().is_some_and(|c| c.eq_ignore_ascii_case(&'l'))
                    && !look.peek().is_some_and(|c| c.is_alphanumeric())
                {
                    chars = look;
                    word.push_str("/dl");
                }
            }
            tokens.push(unit(&word).unwrap_or(&word).to_owned());
        } else if ch == '/' {
            let mut look = chars.clone();
            let next = look.find(|c| !c.is_whitespace());
            let next_is_number = next.is_some_and(|c| c.is_ascii_digit())
                || (next == Some('-') && look.peek().is_some_and(|c| c.is_ascii_digit()));
            if previous_number && next_is_number {
                tokens.push("over".to_owned());
            }
            previous_number = false;
        } else {
            previous_number = false;
            let token = ch.to_string();
            tokens.push(unit(&token).unwrap_or(&token).to_owned());
        }
    }
    if tokens.is_empty() || tokens.iter().all(|t| t.chars().all(is_punctuation)) {
        return Err(VoiceError::TextInvalid);
    }
    Ok(tokens.join(" "))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    #[test]
    fn written_cases() {
        for (input, expected) in [
            ("0", "zero"),
            (
                "120/80 mmHg",
                "one hundred twenty over eighty millimeters of mercury",
            ),
            ("1 / 2", "one over two"),
            ("1/-2", "one over minus two"),
            ("1 / - hello", "one - hello"),
            ("hello / world", "hello world"),
            ("95 bpm", "ninety five beats a minute"),
            ("37.2 °C", "thirty seven point two degrees Celsius"),
            ("0.5", "zero point five"),
            ("-2 kg", "minus two kilograms"),
            (
                "1,200 mg/dL",
                "one thousand two hundred milligrams a deciliter",
            ),
            (
                "999,999",
                "nine hundred ninety nine thousand nine hundred ninety nine",
            ),
            ("07:30", "seven thirty"),
            ("07:05", "seven oh five"),
            ("98% SpO2", "ninety eight percent oxygen saturation"),
            (
                "HRV mmHg ºC h min ml mL",
                "heart rate variability millimeters of mercury degrees Celsius hours minutes milliliters milliliters",
            ),
        ] {
            assert_eq!(normalize_text(input).unwrap(), expected, "{input}");
        }
        for input in ["1000000", "1,20", "25:70", "\t", "/"] {
            assert_eq!(normalize_text(input), Err(VoiceError::TextInvalid));
        }
    }
    #[test]
    #[ignore = "requires MERKEN_KOKORO_DIR with the installed v1.0 package and Merken supplement"]
    fn installed_lexicons_cover_normalizer_vocabulary() {
        let dir = std::env::var_os("MERKEN_KOKORO_DIR")
            .map(std::path::PathBuf::from)
            .expect("set MERKEN_KOKORO_DIR");
        let base = dir.join("lexicon-us-en.txt");
        let supplement = dir.join("merken-us-en.txt");
        let lexicon = Lexicon::load_many(&[&base, &supplement]).unwrap();
        for input in [
            "bpm",
            "mg/dl",
            "mmhg",
            "°c",
            "ºc",
            "%",
            "kg",
            "h",
            "min",
            "ml",
            "spo2",
            "hrv",
            "0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19",
            "20 30 40 50 60 70 80 90 100 1000 -1 0.1 07:05 120/80",
        ] {
            let normalized = normalize_text(input).unwrap();
            let (_, missing) = lexicon.check(&normalized);
            assert!(missing.is_empty(), "{input}: {missing:?}");
        }
    }

    #[test]
    fn lexicon_coverage_and_acronyms() {
        let lexicon = Lexicon::parse("hello || h\ni || i\ns || s\nworld || w\n").unwrap();
        let text = normalize_text("Hello, ISS world! XYZ xyz").unwrap();
        let (prepared, missing) = lexicon.check(&text);
        assert_eq!(prepared, "Hello , I S S world ! XYZ xyz");
        assert_eq!(missing, vec!["XYZ"]);
        let supplemental = Lexicon::parse("# comment\nworld w\n").unwrap();
        assert!(supplemental.check("world").1.is_empty());
        assert!(Lexicon::parse("bad").is_err());
        assert!(Lexicon::parse("").is_err());
    }
}
