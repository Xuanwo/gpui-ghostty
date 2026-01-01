use ghostty_vt::Terminal;
use std::time::Instant;

fn main() {
    let cols: u16 = 120;
    let rows: u16 = 40;
    let mut terminal = Terminal::new(cols, rows).expect("terminal init failed");

    let mut input = String::new();
    input.push_str("\x1b[2J\x1b[H");

    let top = format!("┌{}┐\n", "─".repeat((cols - 2) as usize));
    let mid = format!("│{}│\n", " ".repeat((cols - 2) as usize));
    let bottom = format!("└{}┘\n", "─".repeat((cols - 2) as usize));

    input.push_str(&top);
    for _ in 0..(rows.saturating_sub(3)) {
        input.push_str(&mid);
    }
    input.push_str(&bottom);

    input.push_str("\x1b[1;33mTip:\x1b[0m ");
    input.push_str("Try background terminals for long running processes.\n");
    input.push_str("\x1b[48;5;240m\x1b[38;5;15m> Find and fix a bug in @filename\x1b[0m\n");

    terminal.feed(input.as_bytes()).expect("feed failed");

    let iters: usize = 50;
    let mut total_cell_styles = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        for row in 0..rows {
            total_cell_styles += terminal
                .dump_viewport_row_cell_styles(row)
                .expect("cell style dump failed")
                .len();
        }
    }
    let cell_styles_elapsed = start.elapsed();

    let mut total_style_runs = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        for row in 0..rows {
            total_style_runs += terminal
                .dump_viewport_row_style_runs(row)
                .expect("style run dump failed")
                .len();
        }
    }
    let style_runs_elapsed = start.elapsed();

    println!("iters={iters} rows={rows} cols={cols}");
    println!(
        "cell_styles: {cell_styles_elapsed:?} (records={total_cell_styles})"
    );
    println!("style_runs:  {style_runs_elapsed:?} (records={total_style_runs})");
}

