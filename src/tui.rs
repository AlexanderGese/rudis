use crate::commands;
use crate::resp::Value;
use crate::server::now_ms;
use crate::store::Store;
use crossterm::{event, execute, terminal};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};
use std::io;

struct App {
    store: Store,
    input: String,
    out: Vec<Line<'static>>,
}

// redis-cli-ish rendering of a reply
fn show(v: &Value, depth: usize) -> Vec<Line<'static>> {
    match v {
        Value::Simple(s) => vec![Line::styled(s.clone(), Style::new().fg(Color::Green))],
        Value::Error(s) => vec![Line::styled(format!("(error) {s}"), Style::new().fg(Color::Red))],
        Value::Int(n) => vec![Line::styled(format!("(integer) {n}"), Style::new().fg(Color::Cyan))],
        Value::Bulk(b) => vec![Line::raw(format!("\"{}\"", String::from_utf8_lossy(b)))],
        Value::Null => vec![Line::styled("(nil)", Style::new().fg(Color::DarkGray))],
        Value::Array(a) => {
            if a.is_empty() {
                return vec![Line::styled("(empty array)", Style::new().fg(Color::DarkGray))];
            }
            let mut out = Vec::new();
            for (i, x) in a.iter().enumerate() {
                let mut lines = show(x, depth + 1);
                if let Some(first) = lines.first().cloned() {
                    let prefix = format!("{}) ", i + 1);
                    let mut spans = vec![Span::styled(prefix, Style::new().fg(Color::DarkGray))];
                    spans.extend(first.spans);
                    out.push(Line::from(spans));
                    out.extend(lines.drain(1..));
                }
            }
            out
        }
    }
}

impl App {
    fn run_line(&mut self) {
        let line = self.input.trim().to_string();
        if line.is_empty() {
            return;
        }
        self.out.push(Line::from(vec![
            Span::styled("rudis> ", Style::new().fg(Color::Magenta)),
            Span::raw(line.clone()),
        ]));
        let args: Vec<Vec<u8>> = line.split_whitespace().map(|s| s.as_bytes().to_vec()).collect();
        let reply = commands::dispatch(&mut self.store, &args, now_ms());
        self.out.extend(show(&reply, 0));
        if self.out.len() > 500 {
            let drop = self.out.len() - 500;
            self.out.drain(..drop);
        }
        self.input.clear();
    }
}

pub fn run() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut app = App {
        store: Store::default(),
        input: String::new(),
        out: vec![Line::styled(
            "rudis — try: SET name ada · GET name · RPUSH l a b c · LRANGE l 0 -1 · HSET h f 1 · EXPIRE name 60",
            Style::new().fg(Color::DarkGray),
        )],
    };

    let res = loop {
        if let Err(e) = term.draw(|f| render(f, &app)) {
            break Err(e);
        }
        if let Ok(event::Event::Key(k)) = event::read() {
            if k.kind == event::KeyEventKind::Press {
                match k.code {
                    event::KeyCode::Esc => break Ok(()),
                    event::KeyCode::Enter => app.run_line(),
                    event::KeyCode::Backspace => {
                        app.input.pop();
                    }
                    event::KeyCode::Char(c) => app.input.push(c),
                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    execute!(term.backend_mut(), terminal::LeaveAlternateScreen)?;
    res
}

fn render(f: &mut Frame, app: &App) {
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(f.area());
    let cols = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(rows[0]);

    let inner = cols[0].height.saturating_sub(2) as usize;
    let scroll = app.out.len().saturating_sub(inner) as u16;
    let log = Paragraph::new(app.out.clone())
        .scroll((scroll, 0))
        .block(Block::bordered().title(" repl ").border_style(Style::new().fg(Color::DarkGray)));
    f.render_widget(log, cols[0]);

    f.render_widget(stats(&app.store), cols[1]);

    let prompt = Line::from(vec![
        Span::styled("rudis> ", Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw(&app.input),
        Span::styled("█", Style::new().fg(Color::Gray)),
    ]);
    f.render_widget(
        Paragraph::new(prompt).block(Block::bordered().border_style(Style::new().fg(Color::DarkGray))),
        rows[1],
    );
}

fn stats(store: &Store) -> Paragraph<'static> {
    let (s, l, h) = store.type_counts();
    let total = store.hits + store.misses;
    let rate = if total > 0 { store.hits as f64 / total as f64 * 100.0 } else { 0.0 };
    let lines = vec![
        Line::styled(format!("keys      {}", store.data.len()), Style::new().fg(Color::White)),
        Line::raw(""),
        Line::styled(format!("strings   {s}"), Style::new().fg(Color::Gray)),
        Line::styled(format!("lists     {l}"), Style::new().fg(Color::Gray)),
        Line::styled(format!("hashes    {h}"), Style::new().fg(Color::Gray)),
        Line::raw(""),
        Line::styled(format!("commands  {}", store.cmds), Style::new().fg(Color::Gray)),
        Line::styled(format!("hits      {}", store.hits), Style::new().fg(Color::Green)),
        Line::styled(format!("misses    {}", store.misses), Style::new().fg(Color::Red)),
        Line::styled(format!("hit-rate  {rate:.0}%"), Style::new().fg(Color::Cyan)),
        Line::raw(""),
        Line::styled("Esc to quit", Style::new().fg(Color::DarkGray)),
    ];
    Paragraph::new(lines).block(Block::bordered().title(" keyspace ").border_style(Style::new().fg(Color::DarkGray)))
}
