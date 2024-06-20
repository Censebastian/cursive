//! A simple markup format.
//!
//! # Examples
//!
//! ```cursup
//! /red{This} /green{text} /blue{is} /bold{very} /underline{styled}!
//! /red+bold{This too!}
//! ```
//!
//! ```
//! # use cursive_core as cursive;
//! use cursive::utils::markup::cursup;
//! use cursive::views::Button;
//!
//! // Highlight a letter from the word to show a shortcut available.
//! Button::new(cursup::parse("/red{Q}uit"), |s| s.quit());
//! ```
#![cfg_attr(feature = "doc-cfg", doc(cfg(feature = "cursup")))]

use crate::theme::Style;
use crate::utils::markup::{StyledIndexedSpan, StyledString};
use crate::utils::span::IndexedCow;

use unicode_width::UnicodeWidthStr;

enum State {
    Plain,
    Slash(usize),
}

struct Candidate {
    slash: usize,
    brace: usize,
}

#[derive(Debug)]
enum Event {
    Start(Style),
    End,
    StartSkip,
    Resume,
}

/// Parse spans for the given text.
pub fn parse_spans(input: &str) -> Vec<StyledIndexedSpan> {
    let mut candidates = Vec::<Candidate>::new();
    let mut state = State::Plain;
    let mut events = Vec::new();

    for (i, b) in input.bytes().enumerate() {
        match (&mut state, b) {
            (State::Plain, b'/') => {
                state = State::Slash(i);
            }
            (State::Plain | State::Slash(_), b'}') if !candidates.is_empty() => {
                // Validate this span
                let candidate = candidates.pop().unwrap();

                let action = &input[candidate.slash + 1..candidate.brace];
                let style = action.parse::<Style>().unwrap_or_default();

                events.push((candidate.slash, Event::StartSkip));
                events.push((candidate.brace + 1, Event::Start(style)));
                events.push((i, Event::End));
                events.push((i + 1, Event::Resume));
            }
            (State::Plain, _) => (),

            (State::Slash(_), b'a'..=b'z' | b'+' | b'.') => (),
            (State::Slash(slash), b'{') => {
                // Add a candidate.
                candidates.push(Candidate {
                    slash: *slash,
                    brace: i,
                });
                state = State::Plain;
            }
            (State::Slash(ref mut start), b'/') => {
                // The previous slash is unusable, try with this one.
                *start = i;
            }
            (State::Slash(_), _) => {
                // Unsupported char found.
                state = State::Plain;
            }
        }
    }

    events.sort_by_key(|(i, _)| *i);

    let mut spans = Vec::new();
    let mut style_stack = vec![Style::default()];

    let mut cursor = 0;
    for (i, event) in events {
        match event {
            Event::Start(style) => {
                // Flush everything between cursor and start.
                let new_style = style_stack.last().unwrap().combine(style);
                style_stack.push(new_style);
            }
            Event::StartSkip => {
                // Flush things since cursor
                if cursor != i {
                    spans.push(StyledIndexedSpan {
                        content: IndexedCow::Borrowed {
                            start: cursor,
                            end: i,
                        },
                        attr: *style_stack.last().unwrap(),
                        width: input[cursor..i].width(),
                    });
                }
            }
            Event::End => {
                // Just like StartSkip, but we pop a style from the stack.
                if cursor != i {
                    spans.push(StyledIndexedSpan {
                        content: IndexedCow::Borrowed {
                            start: cursor,
                            end: i,
                        },
                        attr: *style_stack.last().unwrap(),
                        width: input[cursor..i].width(),
                    });
                }
                style_stack.pop();
            }
            Event::Resume => {}
        }
        cursor = i;
    }
    if cursor != input.len() {
        spans.push(StyledIndexedSpan {
            content: IndexedCow::Borrowed {
                start: cursor,
                end: input.len(),
            },
            attr: *style_stack.last().unwrap(),
            width: input[cursor..].width(),
        });
    }

    spans
}

/// Parse the given text into a styled string.
pub fn parse<S>(input: S) -> crate::utils::markup::StyledString
where
    S: Into<String>,
{
    let input = input.into();

    let spans = parse_spans(&input);

    StyledString::with_spans(input, spans)
}

#[cfg(test)]
mod tests {
    use crate::theme::{BaseColor, Effect, Style};
    use crate::utils::markup::cursup::parse;
    use crate::utils::markup::StyledString;
    use crate::utils::span::Span;

    #[test]
    fn empty_string() {
        let parsed = parse("");
        assert_eq!(parsed, StyledString::new());
    }

    #[test]
    fn plain() {
        let parsed = parse("abc");
        assert_eq!(parsed, StyledString::plain("abc"));
    }

    #[test]
    fn single_span() {
        let parsed = parse("/red{red}");
        let spans: Vec<_> = parsed.spans().collect();
        assert_eq!(
            &spans,
            &[Span {
                content: "red",
                width: 3,
                attr: &Style::from_color_style(BaseColor::Red.dark().into())
            }]
        );
    }

    #[test]
    fn span_and_plain() {
        let parsed = parse("/red{Q}uit");
        let spans: Vec<_> = parsed.spans().collect();
        assert_eq!(
            &spans,
            &[
                Span {
                    content: "Q",
                    width: 1,
                    attr: &Style::from_color_style(BaseColor::Red.dark().into())
                },
                Span {
                    content: "uit",
                    width: 3,
                    attr: &Style::default(),
                }
            ]
        );
    }

    #[test]
    fn nested() {
        let parsed = parse("/red{foo /bold{bar} baz}");

        let spans: Vec<_> = parsed.spans().collect();
        assert_eq!(
            spans,
            &[
                Span {
                    content: "foo ",
                    width: 4,
                    attr: &Style::from_color_style(BaseColor::Red.dark().into())
                },
                Span {
                    content: "bar",
                    width: 3,
                    attr: &Style::from_color_style(BaseColor::Red.dark().into())
                        .combine(Effect::Bold)
                },
                Span {
                    content: " baz",
                    width: 4,
                    attr: &Style::from_color_style(BaseColor::Red.dark().into())
                }
            ],
        );
    }

    #[test]
    fn invalid_span_as_plain_text() {
        let parsed = parse("/red{red");
        assert_eq!(parsed, StyledString::plain("/red{red"));
    }
}
