use std::cmp;
use std::collections::HashMap;

use owo_colors::{OwoColorize, Stream::Stdout};
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};
use terminal_size::{Width, terminal_size};

use crate::api::account::{Account, ViewPassword};
use crate::api::entity::Entity;

const DEFAULT_WIDTH: usize = 80;
const TRUNCATION_FLOOR: usize = 40;

pub trait SizeSource {
    fn width(&self) -> usize;
}

pub struct TerminalSize;

impl SizeSource for TerminalSize {
    fn width(&self) -> usize {
        width_or_default(terminal_size().map(|(Width(w), _)| w as usize))
    }
}

fn width_or_default(width: Option<usize>) -> usize {
    width.unwrap_or(DEFAULT_WIDTH)
}

#[cfg(test)]
pub struct FixedWidth(pub usize);

#[cfg(test)]
impl SizeSource for FixedWidth {
    fn width(&self) -> usize {
        self.0
    }
}

pub fn line(account: &Account, size: &dyn SizeSource) -> String {
    let row = format!(
        "{}. {} - {} ({})",
        account.display_id(),
        account.name(),
        account.url().unwrap_or_default().replace("ssh://", ""),
        account.client_name().unwrap_or("")
    )
    .trim()
    .split(' ')
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");

    truncate(&row, size.width().saturating_sub(5))
}

/// The truncation floor applies to `line()` only; table columns follow the source width.
pub fn table(data: &ViewPassword, show: bool, size: &dyn SizeSource) -> String {
    let mut table = Table::new();
    let width = size.width();

    table.max_column_widths = HashMap::from([
        (0, width * 7 / 100),
        (1, width * 29 / 100),
        (2, cmp::max(width * 30 / 100, width - (width * 69 / 100))),
        (3, width * 29 / 100),
    ]);
    table.style = TableStyle::rounded();

    let cells = vec![
        TableCell::new("Id".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Username".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Password".if_supports_color(Stdout, |text| text.green())),
        TableCell::new("Address".if_supports_color(Stdout, |text| text.green())),
    ];

    table.add_row(Row::new(vec![
        TableCell::builder(data.account.name())
            .alignment(Alignment::Center)
            .col_span(cells.len())
            .build(),
    ]));

    table.add_row(Row::new(vec![
        TableCell::builder(data.account.client_name().unwrap_or(""))
            .alignment(Alignment::Center)
            .col_span(cells.len())
            .build(),
    ]));

    table.add_row(Row::new(cells));

    table.add_row(Row::new(vec![
        TableCell::new(data.account.display_id()),
        TableCell::new(data.account.login()),
        TableCell::new(if show { &data.password } else { "Copied to clipboard" }),
        TableCell::new(data.account.url().unwrap_or_default()),
    ]));

    table.render()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(TRUNCATION_FLOOR);
    s.char_indices()
        .nth(max_chars)
        .map_or_else(|| s.to_string(), |(idx, _)| format!("{}...", &s[..idx]))
}

#[cfg(test)]
mod tests {
    use crate::api::account::{Account, ViewPassword};
    use crate::display::{FixedWidth, line, table, truncate, width_or_default};

    fn visible_len(line: &str) -> usize {
        let mut count = 0;
        let mut chars = line.chars();

        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                // ESC plus one byte is a complete escape sequence. ESC + '[' starts a control
                // sequence that runs to its first final byte (0x40-0x7E): 'm' for SGR, but
                // term_table could emit another one, e.g. \e[K.
                match chars.next() {
                    Some('[') => {
                        for c in chars.by_ref() {
                            if ('\u{40}'..='\u{7e}').contains(&c) {
                                break;
                            }
                        }
                    }
                    Some(_) => {}
                    None => break,
                }
                continue;
            }
            count += 1;
        }

        count
    }

    #[test]
    fn test_width_falls_back_to_default_when_terminal_size_is_unavailable() {
        assert_eq!(80, width_or_default(None));
        assert_eq!(60, width_or_default(Some(60)));
    }

    #[test]
    fn test_truncate() {
        let return_text = "add some filler test data that's 40 char...".to_string();
        let test_string = return_text.clone() + " testing long string";

        assert_eq!(return_text, truncate(&test_string, 40));
        assert_eq!(return_text, truncate(&test_string, 1)); // Minimum length is 40
        assert_ne!(return_text, truncate(&test_string, 50));
    }

    #[test]
    fn test_line_at_a_known_width() {
        let size = FixedWidth(80);

        assert_eq!(
            "1. name - example.org (client_name)",
            line(
                &Account::new(
                    Some(1),
                    "name".to_string(),
                    "login".to_string(),
                    Some("ssh://example.org".to_string()),
                    None,
                    0,
                    0,
                    None,
                    Some("client_name".to_string()),
                ),
                &size
            ),
            "input shorter than the floor is not truncated"
        );

        assert_eq!(
            "-. - ()",
            line(
                &Account::new(None, String::new(), String::new(), None, None, 0, 0, None, None),
                &size
            )
        );
    }

    #[test]
    fn test_line_truncates_at_the_floor_when_the_source_is_narrow() {
        let account = Account::new(
            Some(1),
            "add some filler test data that's 40 char".to_string(),
            "login".to_string(),
            Some("ssh://example.org".to_string()),
            None,
            0,
            0,
            None,
            Some("client_name".to_string()),
        );

        assert_eq!(
            "1. add some filler test data that's 40 c...",
            line(&account, &FixedWidth(10)),
            "the floor of 40 chars still applies below the requested width"
        );
    }

    #[test]
    fn test_table_wraps_cells_at_the_source_width() {
        let data = ViewPassword {
            password: "password".to_string(),
            account: Account::new(
                Some(1),
                "name".to_string(),
                "a_long_login_that_needs_wrapping".to_string(),
                Some("ssh://example.org".to_string()),
                None,
                0,
                0,
                None,
                Some("client_name".to_string()),
            ),
        };

        let narrow = table(&data, true, &FixedWidth(60));
        let wide = table(&data, true, &FixedWidth(200));

        assert!(
            narrow.contains("a_long_login_th") && !narrow.contains("a_long_login_that_needs_wrapping"),
            "the login cell must be wrapped, not truncated, at a narrow width: {narrow}"
        );
        assert!(
            wide.contains("a_long_login_that_needs_wrapping"),
            "the login cell fits on one line at a wide width"
        );
        assert!(
            narrow.lines().all(|line| visible_len(line) <= 60),
            "no row may exceed the source width: {narrow}"
        );
    }

    #[test]
    fn test_table_does_not_panic_on_an_account_without_id() {
        let data = ViewPassword {
            password: "password".to_string(),
            account: Account::new(
                None,
                "name".to_string(),
                "login".to_string(),
                None,
                None,
                0,
                0,
                None,
                None,
            ),
        };

        assert!(table(&data, true, &FixedWidth(80)).contains('-'));
    }

    #[test]
    fn test_visible_len_skips_a_non_sgr_control_sequence() {
        assert_eq!(6, visible_len("\u{1b}[31mfoo\u{1b}[Kbar"));
    }
}
