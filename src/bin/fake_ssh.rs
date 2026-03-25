use clap::Parser;
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;

/// Fake SSH binary for testing sshpassx behavior
#[derive(Parser, Debug)]
#[command(name = "fake_ssh")]
struct Args {
    /// Mode to run in
    #[arg(long, default_value = "success")]
    mode: String,

    /// Custom prompt text (used with custom-prompt mode)
    #[arg(long, default_value = "Password: ")]
    prompt: String,

    /// Exit code (used with exit-code mode)
    #[arg(long, default_value_t = 0)]
    exit: i32,
}

fn read_line() -> String {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap_or(0);
    line
}

fn main() {
    let args = Args::parse();
    let stderr = io::stderr();

    match args.mode.as_str() {
        "success" => {
            // Print prompt to stderr, read from stdin, print welcome to stdout, exit 0
            {
                let mut err = stderr.lock();
                write!(err, "Password: ").unwrap();
                err.flush().unwrap();
            }
            let input = read_line();
            if !input.trim().is_empty() {
                println!("Welcome!");
            }
            std::process::exit(0);
        }
        "wrong-password" => {
            // Print prompt, read, print denied, prompt again, read, print denied, exit 1
            {
                let mut err = stderr.lock();
                write!(err, "Password: ").unwrap();
                err.flush().unwrap();
            }
            let _first = read_line();
            {
                let mut err = stderr.lock();
                write!(err, "Permission denied\nPassword: ").unwrap();
                err.flush().unwrap();
            }
            let _second = read_line();
            {
                let mut err = stderr.lock();
                writeln!(err, "Permission denied").unwrap();
                err.flush().unwrap();
            }
            std::process::exit(1);
        }
        "host-key-unknown" => {
            // Print host key unknown message to stderr, exit 0
            {
                let mut err = stderr.lock();
                write!(
                    err,
                    "The authenticity of host 'example.com (1.2.3.4)' can't be established.\nRSA key fingerprint is SHA256:xxx.\nAre you sure you want to continue connecting (yes/no)? "
                )
                .unwrap();
                err.flush().unwrap();
            }
            std::process::exit(0);
        }
        "host-key-changed" => {
            // Print host key changed warning to stderr, exit 0
            {
                let mut err = stderr.lock();
                writeln!(
                    err,
                    "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!\n@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@"
                )
                .unwrap();
                err.flush().unwrap();
            }
            std::process::exit(0);
        }
        "custom-prompt" => {
            // Use --prompt flag value as prompt, read line, exit 0
            {
                let mut err = stderr.lock();
                write!(err, "{}", args.prompt).unwrap();
                err.flush().unwrap();
            }
            let _input = read_line();
            std::process::exit(0);
        }
        "split-prompt" => {
            // Print "Pass" to stderr, flush, sleep 100ms, print "word: ", read line, exit 0
            {
                let mut err = stderr.lock();
                write!(err, "Pass").unwrap();
                err.flush().unwrap();
            }
            thread::sleep(Duration::from_millis(100));
            {
                let mut err = stderr.lock();
                write!(err, "word: ").unwrap();
                err.flush().unwrap();
            }
            let _input = read_line();
            std::process::exit(0);
        }
        "slow-prompt" => {
            // Sleep 2 seconds, then print prompt, read line, exit 0
            thread::sleep(Duration::from_secs(2));
            {
                let mut err = stderr.lock();
                write!(err, "Password: ").unwrap();
                err.flush().unwrap();
            }
            let _input = read_line();
            std::process::exit(0);
        }
        "no-prompt" => {
            // Just exit 0 immediately
            std::process::exit(0);
        }
        "parse-error" => {
            // Print gibberish to stderr, exit 1
            {
                let mut err = stderr.lock();
                writeln!(
                    err,
                    "UNEXPECTED_GIBBERISH_OUTPUT_12345\nSomething went wrong"
                )
                .unwrap();
                err.flush().unwrap();
            }
            std::process::exit(1);
        }
        "exit-code" => {
            // Print prompt, read line, exit with --exit code
            {
                let mut err = stderr.lock();
                write!(err, "Password: ").unwrap();
                err.flush().unwrap();
            }
            let _input = read_line();
            std::process::exit(args.exit);
        }
        unknown => {
            eprintln!("Unknown mode: {}", unknown);
            std::process::exit(2);
        }
    }
}
