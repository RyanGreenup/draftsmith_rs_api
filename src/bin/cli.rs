use clap::Parser;

#[cfg(test)]
use assert_cmd::prelude::*;
#[cfg(test)]
use predicates::prelude::*;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Name of the person to greet
    #[arg(short, long)]
    name: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

fn main() {
    let cli = Cli::parse();

    for _ in 0..cli.count {
        println!("Hello {}!", cli.name);
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;

    #[test]
    fn test_cli_with_name() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.arg("--name").arg("Alice")
            .assert()
            .success()
            .stdout("Hello Alice!\n");
    }

    #[test]
    fn test_cli_with_name_and_count() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.arg("--name").arg("Bob")
            .arg("--count").arg("3")
            .assert()
            .success()
            .stdout("Hello Bob!\nHello Bob!\nHello Bob!\n");
    }

    #[test]
    fn test_cli_missing_name() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.assert()
            .failure()
            .stderr(predicates::str::contains("error: the following required arguments were not provided:"));
    }
}
