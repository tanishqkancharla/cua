//! Exact text matching shared by the Linux Text-interface selection operation.
//! Match in Unicode scalars, then explicitly convert to the toolkit offset unit.
//! The native caller must verify the exact noncollapsed substring before input.
//! This module has no desktop dependencies.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    Text,
    CursorBefore,
    CursorAfter,
}

impl SelectionKind {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "text" => Ok(Self::Text),
            "cursor_before" => Ok(Self::CursorBefore),
            "cursor_after" => Ok(Self::CursorAfter),
            _ => Err("selection_type must be text, cursor_before, or cursor_after"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange {
    pub start: i32,
    pub end: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetUnit {
    UnicodeScalar,
    Utf16,
}

impl OffsetUnit {
    pub fn infer(live: &str, reported_count: i32) -> Result<Self, String> {
        let scalar_count = live.chars().count();
        let utf16_count = live.encode_utf16().count();
        if usize::try_from(reported_count).ok() == Some(scalar_count) {
            Ok(Self::UnicodeScalar)
        } else if usize::try_from(reported_count).ok() == Some(utf16_count) {
            Ok(Self::Utf16)
        } else {
            Err(format!("Text character count has unknown offset units: reported={reported_count}, scalar={scalar_count}, utf16={utf16_count}"))
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::UnicodeScalar => "unicode_scalar",
            Self::Utf16 => "utf16",
        }
    }

    pub fn convert(self, live: &str, range: TextRange) -> Result<TextRange, &'static str> {
        if range.start < 0 || range.end < range.start || range.end as usize > live.chars().count() {
            return Err("matched range is outside live text");
        }
        let offset = |end: i32| -> Result<i32, &'static str> {
            let count = live
                .chars()
                .take(end as usize)
                .map(|c| match self {
                    Self::UnicodeScalar => 1,
                    Self::Utf16 => c.len_utf16(),
                })
                .sum::<usize>();
            i32::try_from(count).map_err(|_| "toolkit text offset exceeds AT-SPI range")
        };
        Ok(TextRange {
            start: offset(range.start)?,
            end: offset(range.end)?,
        })
    }
}

impl TextRange {
    pub fn for_kind(self, kind: SelectionKind) -> Self {
        match kind {
            SelectionKind::Text => self,
            SelectionKind::CursorBefore => Self {
                start: self.start,
                end: self.start,
            },
            SelectionKind::CursorAfter => Self {
                start: self.end,
                end: self.end,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionResult {
    pub range: TextRange,
    pub unit: OffsetUnit,
}

#[derive(Debug)]
pub struct SelectionRequest {
    pub pid: u32,
    pub window_id: u64,
    pub element_index: usize,
    pub element_token: Option<String>,
    pub snapshot_id: Option<String>,
    pub text: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub kind: SelectionKind,
}

#[derive(Debug)]
pub struct SelectionFailure {
    pub mutation_submitted: bool,
    pub message: String,
}

impl std::fmt::Display for SelectionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SelectionFailure {}

pub fn matching_range(
    live_text: &str,
    text: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
    kind: SelectionKind,
) -> Result<TextRange, &'static str> {
    if text.is_empty() {
        return Err("text must not be empty");
    }
    let mut found = None;
    // Consider overlapping matches too: `aa` occurs twice in `aaa`.
    for (start, _) in live_text.char_indices() {
        if !live_text[start..].starts_with(text) {
            continue;
        }
        let end = start + text.len();
        if prefix.is_some_and(|p| !live_text[..start].ends_with(p))
            || suffix.is_some_and(|s| !live_text[end..].starts_with(s))
        {
            continue;
        }
        if found.is_some() {
            return Err("text is ambiguous in the live element; provide prefix/suffix");
        }
        let start = i32::try_from(live_text[..start].chars().count())
            .map_err(|_| "text offset exceeds AT-SPI range")?;
        let end = start
            .checked_add(
                i32::try_from(text.chars().count()).map_err(|_| "text exceeds AT-SPI range")?,
            )
            .ok_or("text offset exceeds AT-SPI range")?;
        found = Some(match kind {
            SelectionKind::Text => TextRange { start, end },
            SelectionKind::CursorBefore => TextRange { start, end: start },
            SelectionKind::CursorAfter => TextRange { start: end, end },
        });
    }
    found.ok_or("text/context was not found in the live element")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_local_to_the_supplied_element_and_count_unicode_scalars() {
        // A non-first paragraph can have this exact local content; no document
        // home operation or document prefix belongs in the resulting offset.
        assert_eq!(
            matching_range("🧪 H2O", "2", None, None, SelectionKind::Text),
            Ok(TextRange { start: 3, end: 4 })
        );
        assert_eq!(
            matching_range("e\u{301}水2", "2", None, None, SelectionKind::Text),
            Ok(TextRange { start: 3, end: 4 })
        );
    }

    #[test]
    fn repeated_and_overlapping_text_requires_disambiguating_context() {
        assert!(matching_range("H2O and CO2", "2", None, None, SelectionKind::Text).is_err());
        assert!(matching_range("aaa", "aa", None, None, SelectionKind::Text).is_err());
        assert_eq!(
            matching_range(
                "H2O and CO2",
                "2",
                Some("CO"),
                Some(""),
                SelectionKind::Text
            ),
            Ok(TextRange { start: 10, end: 11 })
        );
        assert!(matching_range("H2O", "2", Some("CO"), None, SelectionKind::Text).is_err());
    }

    #[test]
    fn toolkit_units_require_an_exact_reported_count() {
        assert_eq!(
            OffsetUnit::infer("a🧪e\u{301}水", 5),
            Ok(OffsetUnit::UnicodeScalar)
        );
        assert_eq!(OffsetUnit::infer("a🧪e\u{301}水", 6), Ok(OffsetUnit::Utf16));
        // BMP and combining characters do not make the two units distinguishable.
        assert_eq!(
            OffsetUnit::infer("e\u{301}水", 3),
            Ok(OffsetUnit::UnicodeScalar)
        );
        let error = OffsetUnit::infer("a🧪", 4).unwrap_err();
        assert!(error.contains("reported=4, scalar=2, utf16=3"));
        assert!(OffsetUnit::infer("a", -1).is_err());
    }

    #[test]
    fn utf16_matches_keep_full_span_until_caret_conversion() {
        let live = "🧪 e\u{301} and 🧪";
        let scalar = matching_range(live, "🧪", Some("and "), None, SelectionKind::Text).unwrap();
        let range = OffsetUnit::Utf16.convert(live, scalar).unwrap();
        assert_eq!(range, TextRange { start: 10, end: 12 });
        assert_eq!(
            range.for_kind(SelectionKind::CursorBefore),
            TextRange { start: 10, end: 10 }
        );
        assert_eq!(
            range.for_kind(SelectionKind::CursorAfter),
            TextRange { start: 12, end: 12 }
        );
        let combining = matching_range(live, "e\u{301}", None, None, SelectionKind::Text).unwrap();
        assert_eq!(
            OffsetUnit::Utf16.convert(live, combining),
            Ok(TextRange { start: 3, end: 5 })
        );
        assert!(OffsetUnit::Utf16
            .convert(live, TextRange { start: 0, end: 99 })
            .is_err());
    }

    #[test]
    fn caret_modes_collapse_at_the_correct_unicode_edge() {
        assert_eq!(
            matching_range("a🧪b", "🧪", None, None, SelectionKind::CursorBefore),
            Ok(TextRange { start: 1, end: 1 })
        );
        assert_eq!(
            matching_range("a🧪b", "🧪", None, None, SelectionKind::CursorAfter),
            Ok(TextRange { start: 2, end: 2 })
        );
        assert!(matching_range("abc", "", None, None, SelectionKind::Text).is_err());
        assert!(matching_range("abc", "missing", None, None, SelectionKind::Text).is_err());
        assert!(SelectionKind::parse("range").is_err());
    }
}
