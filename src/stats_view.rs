//! The statistics page: the worst estimates and the pomodoros of the last days.

use chrono::NaiveDate;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Padding, Paragraph},
};

use crate::{Theme, stats::Stats};

/// How many tasks the "worst estimates" list shows.
pub const WORST_ESTIMATES: usize = 10;
/// How many days the chart shows.
pub const CHART_DAYS: usize = 15;

/// Colour `t` (0.0 to 1.0) of the way from `from` to `to`. Only RGB colours can be blended; any
/// other colour (a named one set in a config, say) is used as it is.
fn blend(from: Color, to: Color, t: f64) -> Color {
    match (from, to) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t.clamp(0.0, 1.0)).round() as u8;
            Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
        }
        _ => from,
    }
}

/// `text` cut to `width` columns (with `…`) and padded with spaces to exactly `width`.
fn fit(text: &str, width: usize) -> String {
    let width_of = |text: &str| Span::raw(text.to_string()).width();
    let mut out = String::new();
    let mut used = 0;
    let too_long = width_of(text) > width;
    let budget = if too_long { width.saturating_sub(1) } else { width };
    for c in text.chars() {
        let w = width_of(&c.to_string());
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    if too_long && width > 0 {
        out.push('…');
        used += 1;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    stats: &Stats,
    preset: Option<&str>,
    today: NaiveDate,
) {
    let block = Block::new()
        .style(Style::new().bg(theme.panel_background))
        .padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [title_area, _, worst_heading, worst_area, _, chart_heading, summary_area, chart_area] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(WORST_ESTIMATES as u16),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(5),
        ])
        .areas(inner);

    let mut title = vec![
        Span::styled("STATISTICS", Style::new().fg(theme.stats_title).bold()),
        Span::styled("  all time", Style::new().fg(theme.stats_dim)),
    ];
    if let Some(preset) = preset {
        title.push(Span::styled(
            format!("  ·  preset {preset}"),
            Style::new().fg(theme.stats_dim),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(title)), title_area);

    draw_worst_estimates(frame, theme, stats, worst_heading, worst_area);
    draw_days(frame, theme, stats, today, chart_heading, summary_area, chart_area);
}

fn heading(theme: &Theme, text: &str, note: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("▌ {text}"), Style::new().fg(theme.stats_heading).bold()),
        Span::styled(format!("  {note}"), Style::new().fg(theme.stats_dim)),
    ])
}

fn draw_worst_estimates(
    frame: &mut Frame,
    theme: &Theme,
    stats: &Stats,
    heading_area: Rect,
    area: Rect,
) {
    frame.render_widget(
        Paragraph::new(heading(
            theme,
            "MOST WRONG ESTIMATES",
            "pomodoros spent over the estimate",
        )),
        heading_area,
    );

    let worst = stats.worst_estimates(WORST_ESTIMATES);
    if worst.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  No task has gone over its estimate yet.",
                Style::new().fg(theme.stats_dim).italic(),
            )),
            area,
        );
        return;
    }

    // rank (2) + gap, name, gap, bar, gap, "est→spent" (7) + gap + "+N" (4)
    let width = area.width as usize;
    let bar_width = (width / 3).clamp(8, 24);
    let name_width = width.saturating_sub(2 + 2 + 2 + bar_width + 2 + 7 + 1 + 4).max(8);
    let scale = worst.iter().map(|w| w.spent).max().unwrap_or(1).max(1) as usize;

    let lines: Vec<Line> = worst
        .iter()
        .enumerate()
        .map(|(i, overrun)| {
            // Worst first: the redder the further over the estimate.
            let heat = blend(
                theme.stats_overrun_worst,
                theme.stats_overrun_mild,
                i as f64 / (WORST_ESTIMATES - 1) as f64,
            );
            let estimated_cells = (overrun.estimated as usize * bar_width / scale).max(1);
            let spent_cells = (overrun.spent as usize * bar_width / scale).max(estimated_cells + 1);
            let over_cells = spent_cells - estimated_cells;

            Line::from(vec![
                Span::styled(format!("{:>2}  ", i + 1), Style::new().fg(heat).bold()),
                Span::styled(
                    fit(&overrun.text, name_width),
                    Style::new().fg(theme.stats_label),
                ),
                Span::raw("  "),
                Span::styled(
                    "█".repeat(estimated_cells),
                    Style::new().fg(theme.stats_estimated),
                ),
                Span::styled("█".repeat(over_cells), Style::new().fg(heat)),
                Span::raw(" ".repeat(bar_width.saturating_sub(spent_cells))),
                Span::raw("  "),
                Span::styled(
                    format!("{:<7}", format!("{}→{}", overrun.estimated, overrun.spent)),
                    Style::new().fg(theme.stats_dim),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("+{}", overrun.excess()),
                    Style::new().fg(heat).bold(),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_days(
    frame: &mut Frame,
    theme: &Theme,
    stats: &Stats,
    today: NaiveDate,
    heading_area: Rect,
    summary_area: Rect,
    area: Rect,
) {
    let days = stats.last_days(today, CHART_DAYS);
    let range = match (days.first(), days.last()) {
        (Some((first, _)), Some((last, _))) => {
            format!("{} – {}", first.format("%b %d"), last.format("%b %d"))
        }
        _ => String::new(),
    };
    frame.render_widget(
        Paragraph::new(heading(
            theme,
            &format!("POMODOROS PER DAY · LAST {CHART_DAYS} DAYS"),
            &range,
        )),
        heading_area,
    );

    let total: u32 = days.iter().map(|(_, count)| count).sum();
    let best = days.iter().max_by_key(|(day, count)| (*count, *day));
    let mut summary = vec![
        Span::styled(format!("{total}"), Style::new().fg(theme.stats_label).bold()),
        Span::styled(" in this period", Style::new().fg(theme.stats_dim)),
        Span::styled("  ·  ", Style::new().fg(theme.stats_dim)),
        Span::styled(
            format!("{:.1}", total as f64 / CHART_DAYS as f64),
            Style::new().fg(theme.stats_label).bold(),
        ),
        Span::styled(" per day", Style::new().fg(theme.stats_dim)),
    ];
    if let Some((day, count)) = best.filter(|(_, count)| *count > 0) {
        summary.push(Span::styled("  ·  best ", Style::new().fg(theme.stats_dim)));
        summary.push(Span::styled(
            format!("{count}"),
            Style::new().fg(theme.stats_label).bold(),
        ));
        summary.push(Span::styled(
            format!(" on {}", day.format("%b %d")),
            Style::new().fg(theme.stats_dim),
        ));
    }
    summary.push(Span::styled("  ·  ", Style::new().fg(theme.stats_dim)));
    summary.push(Span::styled(
        format!("{}", stats.total_pomodoros()),
        Style::new().fg(theme.stats_label).bold(),
    ));
    summary.push(Span::styled(" all time", Style::new().fg(theme.stats_dim)));
    frame.render_widget(Paragraph::new(Line::from(summary)), summary_area);

    // Bars as wide as the space allows, with one column between them.
    let gap = 1u16;
    let bar_width = ((area.width + gap) / CHART_DAYS as u16)
        .saturating_sub(gap)
        .clamp(1, 5);
    let max = days.iter().map(|(_, count)| *count).max().unwrap_or(0).max(1) as u64;

    let bars: Vec<Bar> = days
        .iter()
        .map(|(day, count)| {
            let is_today = *day == today;
            let color = if is_today {
                theme.stats_today
            } else {
                blend(theme.stats_chart_low, theme.stats_chart_high, *count as f64 / max as f64)
            };
            let label_style = if is_today {
                Style::new().fg(theme.stats_today).bold()
            } else {
                Style::new().fg(theme.stats_dim)
            };
            Bar::default()
                .value(*count as u64)
                .text_value(if *count == 0 { String::new() } else { count.to_string() })
                .label(Line::from(day.format("%d").to_string()).style(label_style))
                .style(Style::new().fg(color))
                .value_style(Style::new().fg(theme.stats_bar_text).bg(color).bold())
        })
        .collect();

    frame.render_widget(
        BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(bar_width)
            .bar_gap(gap)
            .max(max),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pads_and_cuts_by_display_width() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("abc", 3), "abc");
        // A wide character takes two columns.
        assert_eq!(fit("日本語", 5), "日本…");
        assert_eq!(fit("", 2), "  ");
    }

    #[test]
    fn blend_mixes_rgb_colors_and_keeps_other_colors() {
        let from = Color::Rgb(0, 100, 200);
        let to = Color::Rgb(100, 200, 0);
        assert_eq!(blend(from, to, 0.0), from);
        assert_eq!(blend(from, to, 1.0), to);
        assert_eq!(blend(from, to, 0.5), Color::Rgb(50, 150, 100));
        assert_eq!(blend(Color::Red, to, 0.5), Color::Red);
    }
}
