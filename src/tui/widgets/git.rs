use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::modules::git::{CommitInfo, RepoStatus};
use crate::tui::theme::Theme;

pub struct GitWidget<'a> {
    repos: &'a [RepoStatus],
    commits: &'a [CommitInfo],
    theme: &'a Theme,
    focused: bool,
}

impl<'a> GitWidget<'a> {
    pub fn new(
        repos: &'a [RepoStatus],
        commits: &'a [CommitInfo],
        theme: &'a Theme,
        focused: bool,
    ) -> Self {
        Self { repos, commits, theme, focused }
    }
}

impl Widget for GitWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("  Git ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.repos.is_empty() && self.commits.is_empty() {
            let text = Paragraph::new("No repositories configured")
                .style(Style::default().fg(self.theme.dim))
                .alignment(Alignment::Center);
            text.render(inner, buf);
            return;
        }

        // Split area between repos and commits
        let chunks = Layout::vertical([
            Constraint::Length((self.repos.len() + 1) as u16),
            Constraint::Min(3),
        ])
        .split(inner);

        self.render_repos(chunks[0], buf);
        self.render_commits(chunks[1], buf);
    }
}

impl GitWidget<'_> {
    fn render_repos(&self, area: Rect, buf: &mut Buffer) {
        if self.repos.is_empty() {
            return;
        }

        let mut y = area.y;

        // Header
        let header = Line::from(vec![
            Span::styled("Repositories", Style::default().fg(self.theme.foreground).add_modifier(Modifier::BOLD)),
        ]);
        Paragraph::new(header).render(Rect::new(area.x, y, area.width, 1), buf);
        y += 1;

        for repo in self.repos.iter().take((area.height - 1) as usize) {
            let branch_icon = if repo.is_clean { "" } else { "" };
            let status_icon = if repo.is_clean { "✓" } else { "●" };
            let status_color = if repo.is_clean {
                self.theme.dim
            } else {
                self.theme.accent
            };

            let mut spans = vec![
                Span::styled(
                    format!("{} ", branch_icon),
                    Style::default().fg(self.theme.foreground),
                ),
                Span::styled(
                    format!("{} ", repo.name),
                    Style::default().fg(self.theme.foreground),
                ),
                Span::styled(
                    format!(" {} ", repo.branch),
                    Style::default().fg(self.theme.dim),
                ),
                Span::styled(
                    status_icon,
                    Style::default().fg(status_color),
                ),
            ];

            // Add ahead/behind indicators
            if repo.ahead > 0 {
                spans.push(Span::styled(
                    format!(" ↑{}", repo.ahead),
                    Style::default().fg(self.theme.accent),
                ));
            }
            if repo.behind > 0 {
                spans.push(Span::styled(
                    format!(" ↓{}", repo.behind),
                    Style::default().fg(self.theme.dim),
                ));
            }

            // Add change counts if dirty
            if !repo.is_clean {
                if repo.modified > 0 {
                    spans.push(Span::styled(
                        format!(" ~{}", repo.modified),
                        Style::default().fg(self.theme.accent),
                    ));
                }
                if repo.staged > 0 {
                    spans.push(Span::styled(
                        format!(" +{}", repo.staged),
                        Style::default().fg(self.theme.foreground),
                    ));
                }
                if repo.untracked > 0 {
                    spans.push(Span::styled(
                        format!(" ?{}", repo.untracked),
                        Style::default().fg(self.theme.dim),
                    ));
                }
            }

            let line = Line::from(spans);
            Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }
    }

    fn render_commits(&self, area: Rect, buf: &mut Buffer) {
        if self.commits.is_empty() {
            return;
        }

        let mut y = area.y;

        // Header
        let header = Line::from(vec![
            Span::styled("Recent Commits", Style::default().fg(self.theme.foreground).add_modifier(Modifier::BOLD)),
        ]);
        Paragraph::new(header).render(Rect::new(area.x, y, area.width, 1), buf);
        y += 1;

        for commit in self.commits.iter().take((area.height - 1) as usize) {
            let hash_short = if commit.hash.len() >= 7 {
                &commit.hash[..7]
            } else {
                &commit.hash
            };

            // Truncate message to fit
            let max_msg_len = (area.width as usize).saturating_sub(30);
            let message = if commit.message.len() > max_msg_len {
                format!("{}…", &commit.message[..max_msg_len.saturating_sub(1)])
            } else {
                commit.message.clone()
            };

            let line = Line::from(vec![
                Span::styled(
                    " ",
                    Style::default().fg(self.theme.foreground),
                ),
                Span::styled(
                    format!("{} ", hash_short),
                    Style::default().fg(self.theme.dim),
                ),
                Span::styled(
                    message,
                    Style::default().fg(self.theme.foreground),
                ),
                Span::styled(
                    format!(" ({})", commit.repo_name),
                    Style::default().fg(self.theme.dim),
                ),
            ]);
            Paragraph::new(line).render(Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }
    }
}

pub struct HelpWidget<'a> {
    theme: &'a Theme,
}

impl<'a> HelpWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for HelpWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.accent))
            .title(" Help ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        let help_text = vec![
            Line::from(vec![
                Span::styled("q / Esc", Style::default().fg(self.theme.accent)),
                Span::styled(" - Quit", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("Space", Style::default().fg(self.theme.accent)),
                Span::styled(" - Play/Pause", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("n", Style::default().fg(self.theme.accent)),
                Span::styled(" - Next track", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("p", Style::default().fg(self.theme.accent)),
                Span::styled(" - Previous track", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("+ / -", Style::default().fg(self.theme.accent)),
                Span::styled(" - Volume up/down", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(self.theme.accent)),
                Span::styled(" - Cycle focus", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("r", Style::default().fg(self.theme.accent)),
                Span::styled(" - Refresh git status", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("l", Style::default().fg(self.theme.accent)),
                Span::styled(" - Toggle lyrics", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("a", Style::default().fg(self.theme.accent)),
                Span::styled(" - Toggle art style", Style::default().fg(self.theme.foreground)),
            ]),
            Line::from(vec![
                Span::styled("?", Style::default().fg(self.theme.accent)),
                Span::styled(" - Toggle help", Style::default().fg(self.theme.foreground)),
            ]),
        ];

        let paragraph = Paragraph::new(help_text);
        paragraph.render(inner, buf);
    }
}
