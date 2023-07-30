use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_RUN_TIME: std::time::Duration = std::time::Duration::from_secs(5);

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    if std::env::args().count() < 3 {
        eprintln!("USAGE: <compiler> <run-dir> [<args>...]");
        std::process::exit(1);
    }
    let compiler_binary = std::env::args().nth(1).unwrap();
    let run_dir = std::env::args().nth(2).unwrap();
    let args: Vec<_> = std::env::args().skip(3).collect();

    let pattern = PathBuf::from(run_dir).join("*.ty");

    let mut num_tests = 0;
    let mut num_passes = 0;
    for ty_file in glob::glob(pattern.to_str().unwrap())? {
        let ty_file = ty_file?;

        num_tests += 1;
        match run_test(&ty_file, &compiler_binary, &args)? {
            TestStatus::Pass => {
                num_passes += 1;
            }
            TestStatus::CompFail(s) => {
                println!("===========================");
                println!("compfail: {}", ty_file.display());
                println!("===========================");
                println!();
                println!("{s}");
            }
            TestStatus::RunFail(s) => {
                println!("===========================");
                println!("runfail: {}", ty_file.display());
                println!("===========================");
                println!("{s}");
            }
        }
    }
    let num_fails = num_tests - num_passes;
    println!("pass: {}", num_passes);
    println!("fail: {}", num_fails);
    println!("total: {}", num_tests);

    if num_fails > 0 {
        std::process::exit(-1)
    }
    Ok(())
}

enum TestStatus {
    Pass,
    RunFail(String),
    CompFail(String),
}

fn run_test(
    ty_path: &Path,
    compiler_binary: &str,
    additional_args: &[String],
) -> Result<TestStatus> {
    let run_compile = Command::new(compiler_binary)
        .arg(&ty_path)
        .args(["-o", "./a.out"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?
        .wait_with_output()?;
    if !run_compile.status.success() {
        return Ok(TestStatus::CompFail(String::from_utf8(
            run_compile.stderr,
        )?));
    }
    let (run_stdout, run_stderr) = {
        let mut process = Command::new("./a.out")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                format!(
                    "error running './a.out' for {}: {e}",
                    ty_path.display()
                )
            })?;

        let mut stdout_str = String::new();
        if let Some(mut stdout) = process.stdout.take() {
            stdout.read_to_string(&mut stdout_str)?;
        }
        let mut stderr_str = String::new();
        if let Some(mut stderr) = process.stderr.take() {
            stderr.read_to_string(&mut stderr_str)?;
        }

        let mut elapsed = std::time::Duration::ZERO;
        while elapsed < MAX_RUN_TIME {
            let now = std::time::Instant::now();
            match process.try_wait() {
                Ok(None) => {}
                Ok(Some(..)) => break,
                Err(..) => panic!(),
            }
            elapsed += now - std::time::Instant::now();
        }
        (stdout_str, stderr_str)
    };
    let stdout_diff = diff_output(ty_path, &run_stdout, "stdout");
    let stderr_diff = diff_output(ty_path, &run_stderr, "stderr");
    let status = if stdout_diff.is_some() || stderr_diff.is_some() {
        TestStatus::RunFail(
            [
                stdout_diff.unwrap_or_default(),
                stderr_diff.unwrap_or_default(),
            ]
            .join("\n"),
        )
    } else {
        TestStatus::Pass
    };

    std::fs::remove_file("./a.out")?;

    Ok(status)
}

fn diff_output(
    base_path: &Path,
    actual: &String,
    ext: &'static str,
) -> Option<String> {
    let expected_path = base_path.with_extension(ext);

    let expected_header = expected_path.as_os_str().to_str().unwrap();
    let actual_header = format!("<{ext}>");

    let expected = read_or_empty_if_not_exist(&expected_path);
    let diff = similar::TextDiff::from_lines(&expected, actual);
    if diff.ratio() == 1.0 {
        return None;
    }
    Some(format!(
        "{}",
        diff.unified_diff()
            .context_radius(1)
            .header(&expected_header, &actual_header)
    ))
}

fn read_or_empty_if_not_exist(path: &PathBuf) -> String {
    match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => String::new(),
            e => panic!("{e:?}"),
        },
    }
}
