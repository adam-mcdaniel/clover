use cage::codegen::ToMage;
use clap::{Parser, ValueEnum};
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use mage::*;
use anyhow::{Result, Context};
use std::{
    fs::File,
    io::Write,
};


#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The input file to compile
    input_file: String,
    /// The output file to write the compiled mage to
    /// If not specified, the output will be written to `main`
    #[arg(short, long, default_value_t = String::from("main"))]
    output_file: String,
    /// The desired target architecture
    #[arg(short, long, value_enum, default_value_t = Backend::C)]
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
    #[arg(short, long)]
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

    let input = std::fs::read_to_string(&args.input_file).context("Failed to read input file")?;
    let cage_program = cage::parse(&input).context("Failed to parse input file")?;
    let mage_program = cage_program.compile_to_mage(&mut cage::Env::default()).context("Failed to compile cage program")?;
    let mage_parsed = mage::parse(&mage_program).context("Failed to parse mage program")?;

    info!("Compiling {} to {} with target {:?}", args.input_file, args.output_file, args.target);

    let output = match args.target {
        Backend::C => {
            let mut c = CCompiler;
            c.compile(mage_parsed)?
        }
        Backend::LLVM => {
            let mut llvm = LLVMCompiler::default();
            llvm.compile(mage_parsed)?
        }
        Backend::Mage => {
            mage_program
        }
        Backend::Interpreter => {
            let i = Interpreter::new(InteractiveInterface);
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

    // Write `src/libcage.c` to the current dir, and add it to the libraries
    std::fs::write(
        "libcage.c",
        include_str!("libcage.c")
    )?;
    args.libraries.push("libcage.c".to_string());

    // Compile the file
    let mut cmd = std::process::Command::new("gcc");
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