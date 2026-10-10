//! Command-line frontend: one-shot evaluation, piped input, or an interactive REPL.

use std::io::{self, BufRead, IsTerminal};
use std::process::ExitCode;

use calc_core::{AngleMode, Calculator, CalcError, Num, MAX_DISPLAY_DIGITS};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
scicalc - arbitrary-precision scientific calculator

USAGE:
    scicalc [OPTIONS] [EXPRESSION]...

With an expression, prints the result and exits (exit status 1 on error).
Without one, starts an interactive session when stdin is a terminal, or
evaluates stdin line by line when it is piped.

OPTIONS:
    -a, --angle <MODE>    Angle mode: deg (default), rad or grad
    -d, --digits <N>      Significant digits to show, 1-60 (default 32)
    -f, --functions       List all functions and exit
    -h, --help            Show this help
    -V, --version         Show the version
    --                    Treat everything after it as the expression

EXAMPLES:
    scicalc \"2+3*4\"
    scicalc \"sin(30) + cos(60)\"
    scicalc -a rad \"sin(pi/6)\"
    scicalc -d 50 \"sqrt(2)\"
    echo \"10!\" | scicalc
";

const REPL_HELP: &str = "\
Type an expression and press Enter.  Operators: + - * / ^ mod ! (and x² x³)
Constants: pi e   Last result: ans   Memory: mem   Variables: x = 5   Functions: f(x) = x^2 + 1

Commands:
    :deg | :rad | :grad     Set the angle mode (:angle shows it)
    :digits [N]             Show or set the significant digits (1-60)
    :ms [expr]              Memory store   (expr, or the last result)
    :m+ [expr]              Memory add     (expr, or the last result)
    :m- [expr]              Memory subtract
    :mr                     Show the memory
    :mc                     Clear the memory
    :history                Show this session's results
    :vars                   Show defined variables and functions
    :clear                  Forget all variables and functions
    :funcs [text]           List functions (optionally filtered by text)
    :help                   Show this help
    :quit                   Leave (Ctrl-D works too)
";

#[derive(Default)]
struct Options {
    angle: Option<AngleMode>,
    digits: Option<usize>,
    list_functions: bool,
    expression: Vec<String>,
}

enum Parsed {
    Run(Options),
    Exit(ExitCode),
}

fn parse_args(args: Vec<String>) -> Parsed {
    let mut opts = Options::default();
    let mut it = args.into_iter();

    fn value_for(flag: &str, inline: Option<String>, it: &mut impl Iterator<Item = String>) -> Result<String, String> {
        inline
            .or_else(|| it.next())
            .ok_or_else(|| format!("option '{flag}' needs a value"))
    }

    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let result: Result<(), String> = match flag.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("scicalc {VERSION}");
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "-f" | "--functions" => {
                opts.list_functions = true;
                Ok(())
            }
            "-a" | "--angle" => value_for(&flag, inline, &mut it)
                .and_then(|v| v.parse::<AngleMode>())
                .map(|m| opts.angle = Some(m)),
            "-d" | "--digits" => value_for(&flag, inline, &mut it).and_then(|v| {
                match v.parse::<usize>() {
                    Ok(n) if (1..=MAX_DISPLAY_DIGITS).contains(&n) => {
                        opts.digits = Some(n);
                        Ok(())
                    }
                    _ => Err(format!("--digits must be a number from 1 to {MAX_DISPLAY_DIGITS}")),
                }
            }),
            "--" => {
                opts.expression.extend(it.by_ref());
                Ok(())
            }
            // Anything else (including "-5+3") belongs to the expression.
            _ => {
                opts.expression.push(arg);
                Ok(())
            }
        };
        if let Err(msg) = result {
            eprintln!("scicalc: {msg}\n\nTry 'scicalc --help'.");
            return Parsed::Exit(ExitCode::from(2));
        }
    }
    Parsed::Run(opts)
}

#[derive(PartialEq)]
enum Flow {
    Continue,
    Quit,
}

fn print_functions(calc: &Calculator, filter: &str) {
    let filter = filter.trim().to_lowercase();
    let mut shown = 0;
    for f in calc.registry().list() {
        if filter.is_empty() || f.name.contains(&filter) || f.help.to_lowercase().contains(&filter) {
            println!("  {:<8} ({:>9} args)  {}", f.name, f.arity.to_string(), f.help);
            shown += 1;
        }
    }
    if shown == 0 {
        println!("No function matches '{filter}'.");
    }
}

fn report(e: &CalcError) {
    eprintln!("{e}");
}

/// Evaluates the optional argument of a memory command, defaulting to `ans`.
fn memory_operand(calc: &mut Calculator, arg: &str) -> Result<Num, CalcError> {
    if arg.trim().is_empty() {
        Ok(calc.last_answer().clone())
    } else {
        calc.eval(arg)
    }
}

fn show_memory(calc: &mut Calculator) {
    match calc.memory().cloned() {
        Some(m) => {
            let s = calc.format(&m);
            println!("memory = {s}");
        }
        None => println!("memory is empty"),
    }
}

fn run_command(calc: &mut Calculator, line: &str) -> Flow {
    let body = line.trim_start_matches(':').trim();
    let (cmd, arg) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    let arg = arg.trim();

    match cmd.to_lowercase().as_str() {
        "q" | "quit" | "exit" => return Flow::Quit,
        "h" | "help" | "?" => print!("{REPL_HELP}"),
        "deg" | "rad" | "grad" => {
            if let Ok(mode) = cmd.parse::<AngleMode>() {
                calc.set_angle_mode(mode);
                println!("angle mode: {mode}");
            }
        }
        "angle" => println!("angle mode: {}", calc.angle_mode()),
        "digits" => {
            if arg.is_empty() {
                println!("digits: {}", calc.display_digits());
            } else {
                match arg.parse::<usize>() {
                    Ok(n) if (1..=MAX_DISPLAY_DIGITS).contains(&n) => {
                        calc.set_display_digits(n);
                        println!("digits: {n}");
                    }
                    _ => eprintln!("digits must be a number from 1 to {MAX_DISPLAY_DIGITS}"),
                }
            }
        }
        "ms" => match memory_operand(calc, arg) {
            Ok(v) => {
                calc.memory_store(v);
                show_memory(calc);
            }
            Err(e) => report(&e),
        },
        "m+" => match memory_operand(calc, arg).and_then(|v| calc.memory_add(&v)) {
            Ok(()) => show_memory(calc),
            Err(e) => report(&e),
        },
        "m-" => match memory_operand(calc, arg).and_then(|v| calc.memory_subtract(&v)) {
            Ok(()) => show_memory(calc),
            Err(e) => report(&e),
        },
        "mr" => show_memory(calc),
        "mc" => {
            calc.memory_clear();
            println!("memory cleared");
        }
        "history" | "hist" => {
            if calc.history().is_empty() {
                println!("(no history yet)");
            }
            for (i, h) in calc.history().iter().enumerate() {
                println!("{:>3}  {} = {}", i + 1, h.input, h.output);
            }
        }
        "vars" => {
            let vars: Vec<(String, Num)> = calc
                .variables()
                .into_iter()
                .map(|(n, v)| (n.to_string(), v.clone()))
                .collect();
            let defs = calc.functions();
            if vars.is_empty() && defs.is_empty() {
                println!("(no variables or functions defined)");
            }
            for (_, text) in &defs {
                println!("{text}");
            }
            for (name, value) in vars {
                let s = calc.format(&value);
                println!("{name} = {s}");
            }
        }
        "clear" => {
            calc.clear_variables();
            calc.clear_functions();
            println!("variables and functions cleared");
        }
        "funcs" | "functions" => print_functions(calc, arg),
        other => eprintln!("unknown command ':{other}' (try :help)"),
    }
    Flow::Continue
}

/// Handles one line of interactive or piped input.
fn handle_line(calc: &mut Calculator, line: &str, result_prefix: &str) -> Result<Flow, ()> {
    let lower = line.to_lowercase();
    if line.starts_with(':') {
        return Ok(run_command(calc, line));
    }
    if matches!(lower.as_str(), "quit" | "exit" | "help" | "?") {
        return Ok(run_command(calc, &lower));
    }
    match calc.define_function(line) {
        Ok(Some(text)) => {
            println!("{result_prefix}defined {text}");
            return Ok(Flow::Continue);
        }
        Ok(None) => {}
        Err(e) => {
            report(&e);
            return Err(());
        }
    }
    match calc.eval_str(line) {
        Ok(s) => {
            println!("{result_prefix}{s}");
            Ok(Flow::Continue)
        }
        Err(e) => {
            report(&e);
            Err(())
        }
    }
}

fn repl(calc: &mut Calculator) -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    println!("scicalc {VERSION} - scientific calculator. Type :help for help, :quit to leave.");
    loop {
        let prompt = format!("{}> ", calc.angle_mode().short_name().to_lowercase());
        match rl.readline(prompt.as_str()) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line);
                if let Ok(Flow::Quit) = handle_line(calc, line, "= ") {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => println!("(use :quit or Ctrl-D to leave)"),
            Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn piped(calc: &mut Calculator) -> ExitCode {
    let mut failed = false;
    for (n, line) in io::stdin().lock().lines().enumerate() {
        let Ok(line) = line else {
            eprintln!("scicalc: could not read stdin");
            return ExitCode::FAILURE;
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match handle_line(calc, line, "") {
            Ok(Flow::Quit) => break,
            Ok(Flow::Continue) => {}
            Err(()) => {
                eprintln!("  (line {})", n + 1);
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let opts = match parse_args(std::env::args().skip(1).collect()) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };

    let mut calc = Calculator::new();
    if let Some(mode) = opts.angle {
        calc.set_angle_mode(mode);
    }
    if let Some(n) = opts.digits {
        calc.set_display_digits(n);
    }

    if opts.list_functions {
        print_functions(&calc, "");
        return ExitCode::SUCCESS;
    }

    if !opts.expression.is_empty() {
        let input = opts.expression.join(" ");
        return match calc.eval_str(&input) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                report(&e);
                ExitCode::FAILURE
            }
        };
    }

    if io::stdin().is_terminal() {
        match repl(&mut calc) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("scicalc: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        piped(&mut calc)
    }
}
