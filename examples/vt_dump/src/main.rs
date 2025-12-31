#[cfg(not(feature = "zig-build"))]
fn main() {
    eprintln!("Enable the `zig-build` feature to build this example.");
}

#[cfg(feature = "zig-build")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;

    let mut t = ghostty_vt::Terminal::new(80, 24)?;
    t.feed(&input)?;

    let s = t.dump_viewport()?;
    print!("{s}");
    Ok(())
}

