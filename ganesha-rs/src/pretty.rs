//! Pretty output formatting for Ganesha
//!
//! Renders LLM responses with:
//! - Markdown formatting (bold, lists, code)
//! - Colorful borders and boxes
//! - Animated elements
//! - Clean typography

use console::{style, Term};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag for bare output mode (no formatting)
static BARE_MODE: AtomicBool = AtomicBool::new(false);

/// Enable bare output mode (raw text only, for scripting)
pub fn set_bare_mode(enabled: bool) {
    BARE_MODE.store(enabled, Ordering::Relaxed);
}

/// Check if bare mode is enabled
pub fn is_bare_mode() -> bool {
    BARE_MODE.load(Ordering::Relaxed)
}

/// Box drawing characters
const BOX_TOP_LEFT: &str = "╭";
const BOX_TOP_RIGHT: &str = "╮";
const BOX_BOTTOM_LEFT: &str = "╰";
const BOX_BOTTOM_RIGHT: &str = "╯";
const BOX_HORIZONTAL: &str = "─";
const BOX_VERTICAL: &str = "│";

/// Terminal width (default if can't detect)
fn term_width() -> usize {
    Term::stdout().size().1 as usize
}

/// Print a pretty bordered box with title
pub fn print_box(title: &str, content: &str) {
    // Bare mode: just print content
    if is_bare_mode() {
        println!("{}", content);
        return;
    }

    let width = term_width().min(100).max(40);
    let inner_width = width - 4; // Account for borders and padding

    // Top border with title
    let title_display = if title.is_empty() {
        String::new()
    } else {
        format!(" {} ", style(title).green().bold())
    };

    let title_len = title.len() + 2; // spaces around title
    let top_width = inner_width + 2; // Match bottom border width
    let remaining = if title_len < top_width {
        top_width - title_len
    } else {
        0
    };
    let left_pad = remaining / 2;
    let right_pad = remaining - left_pad;

    println!(
        "{}{}{}{}{}",
        style(BOX_TOP_LEFT).blue(),
        style(BOX_HORIZONTAL.repeat(left_pad)).blue(),
        title_display,
        style(BOX_HORIZONTAL.repeat(right_pad)).blue(),
        style(BOX_TOP_RIGHT).blue()
    );

    // Content with markdown rendering
    let formatted = render_markdown(content, inner_width);
    for line in formatted.lines() {
        // Ensure line fits within box (truncate if needed)
        let vis_width = visible_width(line);
        let display_line = if vis_width > inner_width {
            truncate_visible(line, inner_width)
        } else {
            line.to_string()
        };

        // Calculate padding needed (based on visible width, not string length)
        let display_vis_width = visible_width(&display_line);
        let padding = if display_vis_width < inner_width {
            inner_width - display_vis_width
        } else {
            0
        };

        println!(
            "{} {}{} {}",
            style(BOX_VERTICAL).blue(),
            display_line,
            " ".repeat(padding),
            style(BOX_VERTICAL).blue()
        );
    }

    // Bottom border
    println!(
        "{}{}{}",
        style(BOX_BOTTOM_LEFT).blue(),
        style(BOX_HORIZONTAL.repeat(inner_width + 2)).blue(),
        style(BOX_BOTTOM_RIGHT).blue()
    );
}

/// Print a simple response without box (for shorter outputs)
pub fn print_response(content: &str) {
    let width = term_width().min(100).max(40);
    let formatted = render_markdown(content, width - 2);
    println!("{}", formatted);
}

/// Render markdown to styled terminal output
pub fn render_markdown(text: &str, max_width: usize) -> String {
    let mut output = String::new();
    let mut in_code_block = false;
    let mut list_depth = 0;

    for line in text.lines() {
        let trimmed = line.trim();

        // Code blocks
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                output.push_str(&format!("{}\n", style("┌──────────────────────────────────────").dim()));
            } else {
                output.push_str(&format!("{}\n", style("└──────────────────────────────────────").dim()));
            }
            continue;
        }

        if in_code_block {
            output.push_str(&format!("{} {}\n", style("│").dim(), style(line).yellow()));
            continue;
        }

        // Headers
        if trimmed.starts_with("### ") {
            let header = &trimmed[4..];
            output.push_str(&format!("\n   {} {}\n", style("▸").cyan(), style(header).bold()));
            continue;
        }
        if trimmed.starts_with("## ") {
            let header = &trimmed[3..];
            output.push_str(&format!("\n  {} {}\n", style("◆").cyan().bold(), style(header).cyan().bold()));
            continue;
        }
        if trimmed.starts_with("# ") {
            let header = &trimmed[2..];
            output.push_str(&format!("\n {} {}\n", style("★").yellow().bold(), style(header).yellow().bold()));
            continue;
        }

        // Bullet points - wrap content if too long
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let content = &trimmed[2..];
            let bullet = match list_depth % 3 {
                0 => "•",
                1 => "◦",
                _ => "▪",
            };
            // Wrap the content (subtract 4 for "  • " prefix)
            let wrap_width = max_width.saturating_sub(4);
            let wrapped = textwrap::fill(content, wrap_width);
            for (i, wrap_line) in wrapped.lines().enumerate() {
                if i == 0 {
                    output.push_str(&format!("  {} {}\n",
                        style(bullet).cyan(),
                        render_inline(wrap_line)));
                } else {
                    output.push_str(&format!("    {}\n", render_inline(wrap_line)));
                }
            }
            continue;
        }

        // Numbered lists - wrap content if too long
        if let Some(rest) = parse_numbered_list(trimmed) {
            let num_prefix = format!("{}.", rest.0);
            // Wrap the content (subtract prefix space)
            let wrap_width = max_width.saturating_sub(5);
            let wrapped = textwrap::fill(rest.1, wrap_width);
            for (i, wrap_line) in wrapped.lines().enumerate() {
                if i == 0 {
                    output.push_str(&format!("  {} {}\n",
                        style(&num_prefix).cyan().bold(),
                        render_inline(wrap_line)));
                } else {
                    output.push_str(&format!("     {}\n", render_inline(wrap_line)));
                }
            }
            continue;
        }

        // Regular paragraph - wrap text before styling
        if !trimmed.is_empty() {
            let wrapped = textwrap::fill(trimmed, max_width);
            for wrap_line in wrapped.lines() {
                output.push_str(&render_inline(wrap_line));
                output.push('\n');
            }
        } else {
            output.push('\n');
        }
    }

    output
}

/// Render inline markdown (bold, italic, code)
fn render_inline(text: &str) -> String {
    let mut result = text.to_string();

    // Bold **text** or __text__
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let bold_text = &result[start + 2..start + 2 + end];
            let styled = format!("{}", style(bold_text).bold());
            result = format!("{}{}{}", &result[..start], styled, &result[start + 4 + end..]);
        } else {
            break;
        }
    }

    // Inline code `code`
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let code_text = &result[start + 1..start + 1 + end];
            let styled = format!("{}", style(code_text).yellow().dim());
            result = format!("{}{}{}", &result[..start], styled, &result[start + 2 + end..]);
        } else {
            break;
        }
    }

    result
}

/// Parse numbered list item (returns number and content)
fn parse_numbered_list(text: &str) -> Option<(u32, &str)> {
    let mut chars = text.chars().peekable();
    let mut num_str = String::new();

    // Parse digits
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    // Must have at least one digit
    if num_str.is_empty() {
        return None;
    }

    // Must be followed by ". " or ") "
    match (chars.next(), chars.next()) {
        (Some('.'), Some(' ')) | (Some(')'), Some(' ')) => {
            let num: u32 = num_str.parse().ok()?;
            let rest: String = chars.collect();
            // This is a bit hacky but we need to return a reference
            // For now, just return the parsed number and use text manipulation
            let content_start = num_str.len() + 2;
            if content_start < text.len() {
                Some((num, &text[content_start..]))
            } else {
                Some((num, ""))
            }
        }
        _ => None,
    }
}

/// Wrap text to max width (strips ANSI codes for accurate width calculation)
fn wrap_text(text: &str, max_width: usize) -> String {
    // Strip ANSI codes for wrapping calculation
    let plain = strip_ansi_codes(text);
    textwrap::fill(&plain, max_width)
}

/// Strip ANSI escape codes from text
fn strip_ansi_codes(text: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(text, "").to_string()
}

/// Calculate visible width of text (ignoring ANSI codes)
fn visible_width(text: &str) -> usize {
    strip_ansi_codes(text).chars().count()
}

/// Truncate text to max visible width, respecting ANSI codes
fn truncate_visible(text: &str, max_width: usize) -> String {
    let plain = strip_ansi_codes(text);
    if plain.chars().count() <= max_width {
        return text.to_string();
    }

    // Simple truncation - just take first max_width chars of plain text
    let truncated: String = plain.chars().take(max_width.saturating_sub(1)).collect();
    format!("{}…", truncated)
}

/// Print a success message with animation
pub fn print_success(message: &str) {
    println!("{} {}", style("✓").green().bold(), style(message).green());
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("{} {}", style("ℹ").cyan(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
    println!("{} {}", style("⚠").yellow(), style(message).yellow());
}

/// Print an error message
pub fn print_error(message: &str) {
    println!("{} {}", style("✗").red().bold(), style(message).red());
}

/// Print a section divider
pub fn print_divider() {
    let width = term_width().min(80);
    println!("{}", style("─".repeat(width)).dim());
}

/// Print Ganesha's response in a nice format
pub fn print_ganesha_response(response: &str) {
    // Unescape common escape sequences (LLMs sometimes output literal \n)
    let response = response
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\"", "\"");

    // Bare mode: just print raw response
    if is_bare_mode() {
        println!("{}", response);
        return;
    }

    // Always use boxed format for consistent output
    println!();
    print_box("Ganesha", &response);
}

/// Animated typing effect for responses
pub fn print_typing(text: &str, delay_ms: u64) {
    use std::thread::sleep;
    use std::time::Duration;

    let term = Term::stdout();
    for c in text.chars() {
        print!("{}", c);
        let _ = std::io::stdout().flush();
        sleep(Duration::from_millis(delay_ms));
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_inline_bold() {
        let result = render_inline("This is **bold** text");
        assert!(result.contains("bold"));
    }

    #[test]
    fn test_parse_numbered_list() {
        assert!(parse_numbered_list("1. First item").is_some());
        assert!(parse_numbered_list("10. Tenth item").is_some());
        assert!(parse_numbered_list("Not a list").is_none());
    }
}
