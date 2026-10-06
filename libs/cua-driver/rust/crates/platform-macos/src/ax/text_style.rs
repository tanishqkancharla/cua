//! Bounded, read-only native text styling for observations. Raw AXValue and
//! UTF-16 selection coordinates never contain these display markers.

use super::bindings::*;
use core_foundation::{
    attributed_string::*,
    base::{CFGetTypeID, CFRange, CFRelease, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionaryGetTypeID, CFDictionaryGetValue, CFDictionaryRef},
    number::CFNumber,
    string::CFString,
};
use std::ptr;

const MAX_TEXT_UNITS: usize = 8192;
const MAX_RUNS: usize = 128;

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateWithName(
        name: core_foundation::string::CFStringRef,
        size: f64,
        matrix: *const std::ffi::c_void,
    ) -> CFTypeRef;
    fn CTFontGetSymbolicTraits(font: CFTypeRef) -> u32;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

#[derive(Clone, Debug)]
struct Run {
    start: usize,
    end: usize,
    style: Style,
}

unsafe fn attribute(dict: CFDictionaryRef, key: &str) -> CFTypeRef {
    let key = CFString::new(key);
    CFDictionaryGetValue(dict, key.as_CFTypeRef())
}

unsafe fn boolean(value: CFTypeRef) -> Option<bool> {
    if value.is_null() {
        return None;
    }
    if CFGetTypeID(value) == CFBoolean::type_id() {
        return Some(CFBoolean::wrap_under_get_rule(value as _).into());
    }
    if CFGetTypeID(value) == CFNumber::type_id() {
        return CFNumber::wrap_under_get_rule(value as _)
            .to_i64()
            .map(|v| v != 0);
    }
    None
}

unsafe fn style(dict: CFDictionaryRef) -> Style {
    let font = attribute(dict, "AXFont");
    let mut traits = 0;
    let mut bold = None;
    let mut italic = None;
    if !font.is_null() && CFGetTypeID(font) == CFDictionaryGetTypeID() {
        // WebKit/system fonts report explicit traits inside AXFont; their
        // private PostScript names need not resolve through CoreText lookup.
        bold = boolean(attribute(font as _, "AXFontBold"));
        italic = boolean(attribute(font as _, "AXFontItalic"));
        let name = attribute(font as _, "AXFontName");
        if !name.is_null() && CFGetTypeID(name) == CFString::type_id() {
            let font = CTFontCreateWithName(name as _, 12.0, ptr::null());
            if !font.is_null() {
                traits = CTFontGetSymbolicTraits(font);
                CFRelease(font);
            }
        }
    }
    Style {
        bold: bold
            .or_else(|| boolean(attribute(dict, "AXFontBold")))
            .unwrap_or(traits & 2 != 0),
        italic: italic
            .or_else(|| boolean(attribute(dict, "AXFontItalic")))
            .unwrap_or(traits & 1 != 0),
        underline: boolean(attribute(dict, "AXUnderline")).unwrap_or(false),
        strike: boolean(attribute(dict, "AXStrikethrough")).unwrap_or(false),
    }
}

/// Unsupported, oversized or inconsistent observations fall back to raw text.
/// The attributed read must match the previously copied AXValue exactly: a
/// concurrently edited or placeholder value cannot acquire stale format runs.
pub unsafe fn copy_formatted_value(
    element: AXUIElementRef,
    role: &str,
    raw: &str,
) -> Option<String> {
    // Native apps can embed a WebArea around their editable text (Notes).
    // The exact AX attributed value contract, not its ancestry, determines
    // whether formatting is observable. Typed browser-page rendering is
    // separate from this Mac AX observation path.
    if !matches!(role, "AXTextArea" | "AXTextField") {
        return None;
    }
    let length = raw.encode_utf16().count();
    if length == 0 || length > MAX_TEXT_UNITS {
        return None;
    }
    let range = AXTextRange {
        location: 0,
        length: length as isize,
    };
    let parameter = AXValueCreate(kAXValueCFRangeType, &range as *const _ as _);
    if parameter.is_null() {
        return None;
    }
    let name = CFString::new("AXAttributedStringForRange");
    let mut value = ptr::null();
    let error = AXUIElementCopyParameterizedAttributeValue(
        element,
        name.as_concrete_TypeRef(),
        parameter as _,
        &mut value,
    );
    CFRelease(parameter as _);
    if error != kAXErrorSuccess || value.is_null() {
        if !value.is_null() {
            CFRelease(value);
        }
        return None;
    }
    if CFGetTypeID(value) != CFAttributedStringGetTypeID() {
        CFRelease(value);
        return None;
    }
    let attributed = CFAttributedString::wrap_under_create_rule(value as _);
    let attributed_ref = attributed.as_concrete_TypeRef();
    let string = CFAttributedStringGetString(attributed_ref);
    if string.is_null()
        || CFGetTypeID(string as _) != CFString::type_id()
        || attributed.char_len() != length as isize
        || CFString::wrap_under_get_rule(string).to_string() != raw
    {
        return None;
    }
    let mut runs = Vec::new();
    let mut cursor = 0;
    while cursor < length {
        if runs.len() >= MAX_RUNS {
            return None;
        }
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        let attributes =
            CFAttributedStringGetAttributes(attributed_ref, cursor as isize, &mut range);
        if attributes.is_null()
            || CFGetTypeID(attributes as _) != CFDictionaryGetTypeID()
            || range.location < 0
            || range.length <= 0
        {
            return None;
        }
        let end = (range.location as usize).checked_add(range.length as usize)?;
        if range.location as usize > cursor || end <= cursor || end > length {
            return None;
        }
        runs.push(Run {
            start: cursor,
            end,
            style: style(attributes),
        });
        cursor = end;
    }
    render(raw, &runs)
}

fn render(raw: &str, runs: &[Run]) -> Option<String> {
    if runs.is_empty() || runs.len() > MAX_RUNS {
        return None;
    }
    // Map only complete Unicode scalar boundaries from UTF-16 into UTF-8.
    let units = raw.encode_utf16().count();
    let mut boundaries = vec![None; units + 1];
    let mut unit = 0;
    for (byte, ch) in raw.char_indices() {
        boundaries[unit] = Some(byte);
        unit += ch.len_utf16();
    }
    boundaries[unit] = Some(raw.len());
    let mut end = 0;
    let mut merged: Vec<Run> = Vec::new();
    for run in runs {
        if run.start != end
            || run.end <= run.start
            || run.end > units
            || boundaries[run.start].is_none()
            || boundaries[run.end].is_none()
        {
            return None;
        }
        if let Some(last) = merged.last_mut().filter(|last| last.style == run.style) {
            last.end = run.end;
        } else {
            merged.push(run.clone());
        }
        end = run.end;
    }
    if end != units {
        return None;
    }
    let visible = raw.trim();
    let start_byte = raw.len() - raw.trim_start().len();
    let end_byte = start_byte + visible.len();
    let mut output = String::new();
    let mut has_style = false;
    for run in merged {
        let start = boundaries[run.start]?.max(start_byte);
        let end = boundaries[run.end]?.min(end_byte);
        if start >= end {
            continue;
        }
        let style = run.style;
        let mut open = String::new();
        let mut close = String::new();
        for (on, left, right) in [
            (style.strike, "~~", "~~"),
            (style.underline, "<u>", "</u>"),
            (style.bold, "**", "**"),
            (style.italic, "*", "*"),
        ] {
            if on {
                open.push_str(left);
                close.insert_str(0, right);
            }
        }
        // Keep paragraph boundaries outside markup so adjacent lines remain
        // distinct even when the same font covers the newline itself.
        for (i, line) in raw[start..end].split('\n').enumerate() {
            if i > 0 {
                output.push('\n');
            }
            if line.is_empty() {
                continue;
            }
            has_style |= style != Style::default();
            output.push_str(&open);
            output.push_str(line);
            output.push_str(&close);
        }
    }
    has_style.then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_foundation::{base::CFType, dictionary::CFDictionary};
    fn bold() -> Style {
        Style {
            bold: true,
            ..Style::default()
        }
    }

    #[test]
    fn merges_font_runs_and_preserves_plain_unicode_fallback() {
        let raw = " Copper 😀 title\nregular ";
        let runs = [
            Run {
                start: 0,
                end: 4,
                style: bold(),
            },
            Run {
                start: 4,
                end: 8,
                style: bold(),
            },
            Run {
                start: 8,
                end: 10,
                style: Style::default(),
            },
            Run {
                start: 10,
                end: 16,
                style: bold(),
            },
            Run {
                start: 16,
                end: 25,
                style: Style::default(),
            },
        ];
        assert_eq!(
            render(raw, &runs).as_deref(),
            Some("**Copper **😀** title**\nregular")
        );
    }

    #[test]
    fn rejects_split_surrogate_gap_overlap_and_partial_coverage() {
        for runs in [
            vec![
                Run {
                    start: 0,
                    end: 1,
                    style: bold(),
                },
                Run {
                    start: 1,
                    end: 3,
                    style: bold(),
                },
            ],
            vec![Run {
                start: 1,
                end: 3,
                style: bold(),
            }],
            vec![Run {
                start: 0,
                end: 2,
                style: bold(),
            }],
            vec![
                Run {
                    start: 0,
                    end: 2,
                    style: bold(),
                },
                Run {
                    start: 1,
                    end: 3,
                    style: bold(),
                },
            ],
        ] {
            assert!(render("😀x", &runs).is_none());
        }
    }

    #[test]
    fn line_boundaries_and_combined_style_are_readable() {
        let style = Style {
            bold: true,
            italic: true,
            underline: true,
            strike: true,
        };
        assert_eq!(
            render(
                "x\n\ny",
                &[Run {
                    start: 0,
                    end: 4,
                    style
                }]
            )
            .as_deref(),
            Some("~~<u>***x***</u>~~\n\n~~<u>***y***</u>~~")
        );
        assert!(render(
            "plain",
            &[Run {
                start: 0,
                end: 5,
                style: Style::default()
            }]
        )
        .is_none());
    }

    #[test]
    fn explicit_nested_traits_win_over_font_lookup_and_root_hints() {
        let font = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
            (
                CFString::new("AXFontName"),
                CFString::new("Helvetica-Bold").as_CFType(),
            ),
            (
                CFString::new("AXFontBold"),
                CFBoolean::false_value().as_CFType(),
            ),
            (
                CFString::new("AXFontItalic"),
                CFBoolean::true_value().as_CFType(),
            ),
        ]);
        let attrs = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
            (CFString::new("AXFont"), font.as_CFType()),
            (
                CFString::new("AXFontBold"),
                CFBoolean::true_value().as_CFType(),
            ),
        ]);
        let observed = unsafe { style(attrs.as_concrete_TypeRef()) };
        assert_eq!(
            observed,
            Style {
                italic: true,
                ..Style::default()
            }
        );
    }

    #[test]
    fn system_font_can_publish_numeric_bold_without_a_resolvable_font_name() {
        let font = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
            (
                CFString::new("AXFontName"),
                CFNumber::from(12_i32).as_CFType(),
            ),
            (
                CFString::new("AXFontBold"),
                CFNumber::from(1_i32).as_CFType(),
            ),
        ]);
        let attrs = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[(
            CFString::new("AXFont"),
            font.as_CFType(),
        )]);
        assert_eq!(unsafe { style(attrs.as_concrete_TypeRef()) }, bold());
    }
}
