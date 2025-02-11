use cage::{codegen::ToMage, WithMetadata, CageInterface, Env};
use clap::{Parser, ValueEnum};
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use mage::*;
use anyhow::{Result, Context};
use std::{
    borrow::Cow, fmt::{Display, Write as FmtWrite}, fs::File, io::Write
};
use crossterm::cursor;
use rustyline::{completion::Completer, highlight::{CmdKind, Highlighter}, hint::Hinter, validate::Validator, Helper};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The input file to compile
    input_file: Option<String>,
    /// The output file to write the compiled mage to
    /// If not specified, the output will be written to `main`
    #[arg(short, long, default_value_t = String::from("main"))]
    output_file: String,
    /// The desired target architecture
    #[arg(short, long, value_enum, default_value_t = Backend::LLVM)]
    target: Backend,
    /// Whether or not to include debug information
    #[arg(short, long)]
    debug: bool,
    /// Whether or not to use the address sanitizer
    #[arg(short, long)]
    asan: bool,
    /// Whether or not to use release optimizations
    #[arg(short, long)]
    release: bool,
    /// The libraries to link against
    #[arg(short='L', long, )]
    libraries: Vec<String>,
}

/// The target architecture to compile to
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Backend {
    /// Use LLVM to compile to the host architecture
    #[value(alias("llvm"), alias("l"))]
    LLVM,
    #[value(alias("c"))]
    #[default]
    C,
    #[value(alias("m"), alias("mg"))]
    Mage,
    #[value(alias("i"), alias("int"))]
    Interpreter,
}

fn clear_screen() {
    print!("\x1b[2J\x1b[1;1H");
}


#[allow(dead_code)]
fn print_rainbow_text(text: &str) {
    let colors = [
        "\x1b[31m", // Red
        "\x1b[33m", // Yellow
        "\x1b[32m", // Green
        "\x1b[36m", // Cyan
        "\x1b[34m", // Blue
        "\x1b[35m", // Magenta
    ];

    let mut color_index = 0;
    let mut rainbow_text = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            write!(rainbow_text, "{}", ch).unwrap(); // Preserve whitespace
        } else {
            write!(rainbow_text, "{}{}", colors[color_index], ch).unwrap();
            color_index = (color_index + 1) % colors.len();
        }
    }

    rainbow_text.push_str("\x1b[0m"); // Reset color
    println!("{}", rainbow_text);
}

#[allow(dead_code)]
fn print_rainbow_text_lines(text: &str) {
    let colors = [
        "\x1b[31m", // Red
        "\x1b[33m", // Yellow
        "\x1b[32m", // Green
        "\x1b[36m", // Cyan
        "\x1b[34m", // Blue
        "\x1b[35m", // Magenta,
    ];

    let mut color_index = 2;
    let mut rainbow_text = String::new();

    for ch in text.chars() {
        if ch == '\n' {
            color_index = (color_index + 1) % colors.len();
        }
        write!(rainbow_text, "{}{}", colors[color_index], ch).unwrap();
    }

    rainbow_text.push_str("\x1b[0m"); // Reset color
    println!("{}", rainbow_text);
}

#[allow(dead_code)]
fn ansi_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

#[allow(dead_code)]
fn ansi_hsv(h: f64, s: f64, v: f64) -> String {
    let (r, g, b) = hsv_to_rgb((h, s, v));
    ansi_rgb(r, g, b)
}

#[allow(dead_code)]
fn ansi_rgb_bg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{};{};{}m", r, g, b)
}

#[allow(dead_code)]
fn ansi_hsv_bg(h: f64, s: f64, v: f64) -> String {
    let (r, g, b) = hsv_to_rgb((h, s, v));
    ansi_rgb_bg(r, g, b)
}

#[allow(dead_code)]
fn ansi_reset() -> String {
    "\x1b[0m".to_string()
}


#[allow(dead_code)]
fn hsv_to_rgb(c: (f64, f64, f64)) -> (u8, u8, u8) {
    let (h, s, v) = c;

    let c = v * s; // Chroma
    let h_prime = h / 60.0; // Sector of the color wheel
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if (0.0..1.0).contains(&h_prime) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h_prime) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h_prime) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h_prime) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h_prime) {
        (x, 0.0, c)
    } else if (5.0..6.0).contains(&h_prime) {
        (c, 0.0, x)
    } else {
        (0.0, 0.0, 0.0) // Fallback for invalid hue
    };

    let r = ((r + m) * 255.0).round() as u8;
    let g = ((g + m) * 255.0).round() as u8;
    let b = ((b + m) * 255.0).round() as u8;

    (r, g, b)
}

#[allow(dead_code)]
fn rgb_to_hsv(c: (u8, u8, u8)) -> (f64, f64, f64) {
    let (r, g, b) = c;
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Calculate hue
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    // Calculate saturation
    let s = if max == 0.0 { 0.0 } else { delta / max };

    // Calculate value
    let v = max;

    (h, s, v)
}


fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            while let Some(ch) = chars.next() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}
#[allow(dead_code)]
fn gradient(text: impl Display, color1: (f64, f64, f64), color2: (f64, f64, f64), fg: bool) -> String {
    let mut result = String::new();
    let text = text.to_string();
    let text = strip_ansi_codes(&text);
    

    let (mut hue, mut saturation, mut value) = color1;
    let char_count = text.chars().filter(|c| !c.is_whitespace() || !fg).count();
    // let start_hue = color1.0.min(color2.0);
    // let end_hue = color1.0.max(color2.0);
    let start_hue = color1.0;
    let end_hue = color2.0;
    let hue_step = (end_hue - start_hue) / char_count as f64;

    // let start_saturation = color1.1.min(color2.1);
    // let end_saturation = color1.1.max(color2.1);
    let start_saturation = color1.1;
    let end_saturation = color2.1;
    let saturation_step = (end_saturation - start_saturation) / char_count as f64;

    // let start_value = color1.2.min(color2.2);
    // let end_value = color1.2.max(color2.2);
    let start_value = color1.2;
    let end_value = color2.2;
    let value_step = (end_value - start_value) / char_count as f64;

    for ch in text.chars() {
        if ch == '\n' {
            // Print reset
            result.push_str(&ansi_reset());
            result.push(ch);
        } else if ch.is_whitespace() && fg {
            result.push(ch);
        } else {
            let ansi = if fg {
                ansi_hsv(hue, saturation, value)
            } else {
                ansi_hsv_bg(hue, saturation, value)
            };
            
            result.push_str(&ansi);
            result.push(ch);
            result.push_str(&ansi_reset());

            hue += hue_step;
            saturation += saturation_step;
            value += value_step;
        }
    }

    result
}

fn rgb_text(text: impl Display, color: (u8, u8, u8)) -> String {
    let text = text.to_string();
    let (hue, saturation, value) = color;
    let ansi = ansi_rgb(hue, saturation, value);
    let ansi_reset = ansi_reset();

    format!("{}{}{}", ansi, text, ansi_reset)
}

fn eval(mut input: String, i: &mut Interpreter<CageInterface>, env: &mut Env) -> Result<()> {
    let mut last_input = String::new();
    while input != last_input && !input.trim().is_empty() {
        last_input = input.clone();
        let (rest, stmt) = cage::parse_single_stmt(&input).context("Failed to parse input")?;
        input = rest.to_string();
        let mage_program = stmt.to_mage(env).with_metadata("Could not compile to mage")?;
        let mage_parsed = mage::parse(&mage_program).context("Failed to parse mage")?;
        i.partial_run(&mage_parsed).context("Failed to run program")?;
    }
    Ok(())
}

fn is_valid_input(input: &str) -> bool {
    cage::parse(input).is_ok()
}

fn get_cursor_position() -> std::io::Result<(u16, u16)> {
    let (col, row) = cursor::position()?;
    Ok((row + 1, col + 1)) // Adjust to 1-based indexing
}

fn move_to_home() -> std::io::Result<()> {
    if get_cursor_position()?.1 != 1 {
        println!();
    }
    Ok(())
}

/// Custom Highlighter for Rustyline
struct MyHighlighter;

impl Highlighter for MyHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let control_flow = ["if", "else", "while", "continue", "break", "return"];
        let declarations = ["let", "mut", "type", "struct", "union", "fun", "extern"];
        let types = ["Int", "Float", "Bool", "String", "Char", "Cell"];

        let reset = "\x1b[0m";  // Reset color
        let control_color = "\x1b[33m";  // Yellow for control flow
        let decl_color = "\x1b[34m";  // Blue for declarations
        let type_color = "\x1b[32m";  // Green for types

        let mut highlighted = String::new();
        let mut buffer = String::new(); // Buffer to store a word
        let mut in_word = false;

        for c in line.chars() {
            if c.is_whitespace() || c.is_ascii_punctuation() {
                if in_word {
                    // Apply highlighting if buffer matches a keyword
                    if control_flow.contains(&buffer.as_str()) {
                        highlighted.push_str(control_color);
                    } else if declarations.contains(&buffer.as_str()) {
                        highlighted.push_str(decl_color);
                    } else if types.contains(&buffer.as_str()) {
                        highlighted.push_str(type_color);
                    }

                    highlighted.push_str(&buffer);
                    highlighted.push_str(reset);
                    buffer.clear();
                    in_word = false;
                }
                highlighted.push(c); // Preserve whitespace
            } else {
                buffer.push(c);
                in_word = true;
            }
        }

        // Check for last buffered word
        if !buffer.is_empty() {
            if control_flow.contains(&buffer.as_str()) {
                highlighted.push_str(control_color);
            } else if declarations.contains(&buffer.as_str()) {
                highlighted.push_str(decl_color);
            } else if types.contains(&buffer.as_str()) {
                highlighted.push_str(type_color);
            }
            highlighted.push_str(&buffer);
            highlighted.push_str(reset);
        }

        Cow::Owned(highlighted)
    }
    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true
    }
}

// Implement Helper trait (required for Rustyline custom helpers)
impl Helper for MyHighlighter {}
impl Completer for MyHighlighter {
    type Candidate = String;
}
impl Hinter for MyHighlighter {
    type Hint = String;
}
impl Validator for MyHighlighter {}

fn repl() -> Result<()> {
    use rustyline::error::ReadlineError;
    clear_screen();
    let mut env = Env::default();
    let mut i = Interpreter::new(CageInterface);    
    i.partial_run(&mage::parse(cage::codegen::MAGE_PRELUDE)?)?;
    println!("\nUse the REPL to enter your program statements.\nFor help, input \":help\"\n");

    let default_entry = rgb_text(">>> ", (0, 255, 0));
    let continuation_entry = rgb_text("... ", (0, 0, 255));
    if let Ok(mut rl) = rustyline::Editor::new() {
        rl.set_helper(Some(MyHighlighter));

        rl.load_history(".cage_history").ok();
        let mut current_input = String::new();
        let mut current_prompt = default_entry.clone();
        loop {
            let readline = rl.readline(&current_prompt);
            if rl.save_history(".cage_history").is_err() {
                error!("Could not save history");
            }
            match readline {
                Ok(line) => {
                    if line.trim().chars().next() == Some(':') {
                        let line = line.trim();
                        if line == ":q" || line == ":quit" {
                            break;
                        } else if line == ":c" || line == ":clear" {
                            env = Env::default();
                            i = Interpreter::new(CageInterface);
                            info!("Environment cleared.");
                            continue;
                        } else if line == ":x" || line == ":examine" {
                            println!("{:?}", env);
                            continue;
                        } else if line == ":i" || line == ":import" {
                            // Ask for a filename
                            let filename = rl.readline("Enter the filename to import: ");
                            if let Ok(filename) = filename {
                                if let Ok(program) = std::fs::read_to_string(&filename) {
                                    current_input.push_str(&program);
                                    match eval(current_input.clone(), &mut i, &mut env) {
                                        Ok(_) => {
                                            info!("OK.");
                                        },
                                        Err(error) => {
                                            for line in error.to_string().lines() {
                                                error!("{}", line);
                                            }
                                        }
                                    }
                                    current_input.clear();
                                    current_prompt = default_entry.clone();
                                    continue;
                                }
                            }
                        } else if line == ":h" || line == ":help" {
                            println!("Commands:");
                            println!("  :q, :quit    - Quit the REPL");
                            println!("  :x, :examine - Print the environment");
                            println!("  :c, :clear   - Reset the environment");
                            println!("  :h, :help    - Show this help message");
                            continue;
                        }
                    }


                    current_input.push_str(&line);
                    current_input.push_str("\n");
                    let _ = rl.add_history_entry(&line);
                    if (is_valid_input(&current_input) || line == "") && !current_input.trim().is_empty() {
                        match eval(current_input.clone(), &mut i, &mut env) {
                            Ok(_) => {
                                let _ = move_to_home();
                            },
                            Err(error) => {
                                for line in error.to_string().lines() {
                                    error!("{}", line);
                                }
                            }
                        }
                        let _ = rl.add_history_entry(current_input.trim());
                        current_input.clear();
                        current_prompt = default_entry.clone();
                    } else if !current_input.trim().is_empty() {
                        current_prompt = continuation_entry.clone();
                    } else {
                        current_prompt = default_entry.clone();
                        current_input.clear();
                    }
                },
                Err(ReadlineError::Interrupted) => {
                    current_input.clear();
                    current_prompt = default_entry.clone();
                },
                Err(ReadlineError::Eof) => {
                    // Save the history
                    let _ = rl.save_history(".reckon_history");
                    break
                },
                Err(_) => {
                    error!("Could not read input");
                    break
                }
            }
        }
    } else {
        error!("Could not initialize readline");
    }

    Ok(())
}

fn main() -> Result<()> {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::INFO)
        // completes the builder.
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");

    let mut args = Args::parse();
    if args.input_file == None {
        return repl();
    }

    let input_file = args.input_file.take().unwrap();
    let input = std::fs::read_to_string(&input_file).context("Failed to read input file")?;
    let cage_program = cage::parse(&input).context("Failed to parse input file")?;
    let mage_program = cage_program.compile_to_mage(&mut cage::Env::default()).context("Failed to compile cage program")?;
    let mage_parsed = mage::parse(&mage_program).context("Failed to parse mage program")?;

    info!("Compiling {} to {} with target {:?}", input_file, args.output_file, args.target);

    let output = match args.target {
        Backend::C => {
            info!("Compiling with Clang...");
            let mut c = CCompiler;
            c.compile(mage_parsed)?
        }
        Backend::LLVM => {
            info!("Compiling with LLVM...");
            let mut llvm = LLVMCompiler::default();
            llvm.compile(mage_parsed)?
        }
        Backend::Mage => {
            mage_program
        }
        Backend::Interpreter => {
            let i = Interpreter::new(CageInterface);
            let _interface = i.run(&mage_parsed).context("Failed to run program")?;
            "".to_string()
        }
    };

    if args.target == Backend::Interpreter {
        return Ok(())
    }

    let path = std::path::Path::new(&args.output_file).with_extension(
        match args.target {
            Backend::C => "c",
            Backend::LLVM => "ll",
            Backend::Mage => "mg",
            Backend::Interpreter => unreachable!(),
        }
    );
    let mut file = File::create(&path)?;
    write!(file, "{}", output)?;

    if args.target == Backend::Mage {
        info!("Successfully compiled {}", path.display());
        return Ok(());
    }

    // Write `src/libcage.c` to the current dir, and add it to the libraries
    std::fs::write(
        "libcage.c",
        include_str!("libcage.c")
    )?;
    args.libraries.push("libcage.c".to_string());

    // Compile the file
    let mut cmd = std::process::Command::new("clang");
    if args.libraries.iter().any(|lib| lib.ends_with(".c")) {
        cmd
            .arg(&path)
            .args(&args.libraries);
    } else {
        cmd
            .arg(&path);
    }
    if args.debug {
        cmd.arg("-g");
        info!("Compiling with debug information");
    }
    if args.release {
        cmd.arg("-O3");
        info!("Compiling with release optimizations");
    }
    if args.asan {
        cmd.arg("-fsanitize=address");
        info!("Compiling with address sanitizer");
    }
    let status = cmd.arg("-o")
        .arg(path.with_extension("exe"))
        .status()?;

    // Remove libcage.c
    std::fs::remove_file("libcage.c")?;
    if !status.success() {
        error!("Failed to compile {}", path.display());
        return Err(anyhow::anyhow!("Failed to compile {}", path.display()));
    } else {
        info!("Successfully compiled {}", path.display());
    }
    Ok(())
}