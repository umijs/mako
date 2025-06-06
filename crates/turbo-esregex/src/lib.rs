#![feature(arbitrary_self_types_pointers)]

use anyhow::{Result, bail};

pub fn register() {
    turbo_tasks::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}

/// A simple regular expression implementation following ecmascript semantics
///
/// Delegates to the `regex` crate when possible and `regress` otherwise.
#[derive(Debug, Clone)]
#[turbo_tasks::value(eq = "manual", shared)]
#[serde(into = "RegexForm", try_from = "RegexForm")]
pub struct EsRegex {
    #[turbo_tasks(trace_ignore)]
    delegate: EsRegexImpl,
    // Store the original arguments used to construct
    // this regex to support equality and serialization.
    pattern: String,
    flags: String,
}

#[derive(Debug, Clone)]
enum EsRegexImpl {
    Regex(regex::Regex),
    Regress(regress::Regex),
}

/// Equality uses the source inputs since our delegate regex impls don't support
/// equality natively.
/// NOTE: there are multiple 'equivalent' ways to write a regex and this
/// approach does _not_ attempt to equate them.
impl PartialEq for EsRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.flags == other.flags
    }
}
impl Eq for EsRegex {}

impl TryFrom<RegexForm> for EsRegex {
    type Error = anyhow::Error;

    fn try_from(value: RegexForm) -> std::result::Result<Self, Self::Error> {
        EsRegex::new(&value.pattern, &value.flags)
    }
}

/// This is the serializable form for the `EsRegex` struct
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct RegexForm {
    pattern: String,
    flags: String,
}

impl From<EsRegex> for RegexForm {
    fn from(value: EsRegex) -> Self {
        Self {
            pattern: value.pattern,
            flags: value.flags,
        }
    }
}

impl EsRegex {
    /// Support ecmascript style regular expressions by selecting the `regex` crate when possible
    /// and using regress when not.
    pub fn new(pattern: &str, flags: &str) -> Result<Self> {
        // rust regex doesn't allow escaped slashes, but they are necessary in js
        let pattern = pattern.replace("\\/", "/");

        let mut applied_flags = String::new();
        for flag in flags.chars() {
            match flag {
                // indices for substring matches: not relevant for the regex itself
                'd' => {}
                // global: default in rust, ignore
                'g' => {}
                // case-insensitive: letters match both upper and lower case
                'i' => applied_flags.push('i'),
                // multi-line mode: ^ and $ match begin/end of line
                'm' => applied_flags.push('m'),
                // allow . to match \n
                's' => applied_flags.push('s'),
                // Unicode support (enabled by default)
                'u' => applied_flags.push('u'),
                // sticky search: not relevant for the regex itself
                'y' => {}
                _ => bail!("unsupported flag `{flag}` in regex: `{pattern}` with flags: `{flags}`"),
            }
        }

        let regex = if !applied_flags.is_empty() {
            regex::Regex::new(&format!("(?{applied_flags}){pattern}"))
        } else {
            regex::Regex::new(&pattern)
        };

        let delegate = match regex {
            Ok(reg) => Ok(EsRegexImpl::Regex(reg)),
            Err(_e) => {
                // We failed to parse as an regex:Regex, try using regress. Regress uses the es
                // flags format so we can pass the original flags value.
                match regress::Regex::with_flags(&pattern, regress::Flags::from(flags)) {
                    Ok(reg) => Ok(EsRegexImpl::Regress(reg)),
                    // Propogate the error as is, regress has useful error messages.
                    Err(e) => Err(e),
                }
            }
        }?;
        Ok(Self {
            delegate,
            pattern,
            flags: flags.to_string(),
        })
    }

    /// Returns true if there is any match for this regex in the `haystac`
    pub fn is_match(&self, haystack: &str) -> bool {
        match &self.delegate {
            EsRegexImpl::Regex(r) => r.is_match(haystack),
            EsRegexImpl::Regress(r) => r.find(haystack).is_some(),
        }
    }

    pub fn captures(&self, haystack: &str) -> Option<Vec<String>> {
        match &self.delegate {
            EsRegexImpl::Regex(r) => r.captures(haystack).map(|caps| {
                caps.iter()
                    .map(|m| m.map(|m| m.as_str().to_string()).unwrap_or_default())
                    .collect()
            }),
            EsRegexImpl::Regress(r) => {
                r.find(haystack).map(|m| {
                    let mut captures = Vec::new();
                    // Add the full match (group 0)
                    captures.push(haystack[m.range()].to_string());

                    // Add capture groups (group 1, 2, 3, ...)
                    let mut group_index = 1;
                    while let Some(group_range) = m.group(group_index) {
                        captures.push(haystack[group_range].to_string());
                        group_index += 1;
                    }

                    captures
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EsRegex, EsRegexImpl};

    #[test]
    fn es_regex_matches_simple() {
        let regex = EsRegex::new("a", "").unwrap();
        assert!(matches!(regex.delegate, EsRegexImpl::Regex { .. }));
        assert!(regex.is_match("a"));
    }

    #[test]
    fn es_regex_matches_negative_lookahead() {
        // This feature is not supported by the regex crate
        let regex = EsRegex::new("a(?!b)", "").unwrap();
        assert!(matches!(regex.delegate, EsRegexImpl::Regress { .. }));
        assert!(!regex.is_match("ab"));
        assert!(regex.is_match("ac"));
    }

    #[test]
    fn invalid_regex() {
        // This is invalid since there is nothing being repeated
        // Don't bother asserting on the message since we delegate
        // that to the underlying implementations.
        assert!(matches!(EsRegex::new("*", ""), Err { .. }))
    }

    #[test]
    fn captures_with_regex() {
        let regex = EsRegex::new(r"(\d{4})-(\d{2})-(\d{2})", "").unwrap();
        assert!(matches!(regex.delegate, EsRegexImpl::Regex { .. }));

        let captures = regex.captures("Today is 2024-01-15");
        assert!(captures.is_some());
        let caps = captures.unwrap();
        assert_eq!(caps.len(), 4); // full match + 3 groups
        assert_eq!(caps[0], "2024-01-15"); // full match
        assert_eq!(caps[1], "2024"); // year
        assert_eq!(caps[2], "01"); // month
        assert_eq!(caps[3], "15"); // day
    }

    #[test]
    fn captures_with_regress() {
        // Using negative lookahead to force regress usage
        let regex = EsRegex::new(r"(\w+)(?!bar)", "").unwrap();
        assert!(matches!(regex.delegate, EsRegexImpl::Regress { .. }));

        let captures = regex.captures("foobar");
        assert!(captures.is_none()); // Should not match due to negative lookahead

        let captures = regex.captures("foobaz");
        assert!(captures.is_some());
        let caps = captures.unwrap();
        assert_eq!(caps.len(), 2); // full match + 1 group
        assert_eq!(caps[0], "foo"); // full match
        assert_eq!(caps[1], "foo"); // captured group
    }
}
