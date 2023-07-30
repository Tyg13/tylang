type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let task = std::env::args().nth(1);
    match task.as_ref().map(String::as_str) {
        Some("ci") => ci(),
        _ => Ok(()),
    }
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn build_package(cargo: &str, proj: &str) -> Result<(), Error> {
    let build = std::process::Command::new(cargo)
        .args(&["build", "--package", proj])
        .status()?;
    if !build.success() {
        Err(format!("cargo build '{proj}' failed!"))?;
    }
    Ok(())
}

fn ci() -> Result<(), Error> {
    let c = cargo();
    build_package(&c, "testc")?;
    build_package(&c, "tyc")?;

    let unit_tests_pass =
        std::process::Command::new(c).args(&["test"]).status()?;
    let testc_tests_pass = std::process::Command::new("target/debug/testc")
        .args(&["target/debug/tyc", "tests/"])
        .status()?;

    if !unit_tests_pass.success() || !testc_tests_pass.success() {
        std::process::exit(-1)
    }
    Ok(())
}
