use pretty_assertions::assert_eq;
use std::fs;
use std::io::Write;
use std::process::{exit, Command};
use std::{fmt, path::PathBuf};

pub fn assert_contains_part(left: &str, right: &str) {
  let left_no_ws = left.split_whitespace().collect::<String>();
  let right_no_ws = right.split_whitespace().collect::<String>();
  if !left_no_ws.contains(&right_no_ws) {
    assert_eq!(
      PrettyString(left),
      PrettyString(right),
      "\n\nThe left hand argument to assert_contains_part does not contain the right hand:\n\n",
    );
  }
}

#[derive(PartialEq, Eq)]
#[doc(hidden)]
pub struct PrettyString<'a>(pub &'a str);

/// Make diff to display string as multi-line string
impl<'a> fmt::Debug for PrettyString<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(self.0)
  }
}

pub fn build_lib(path: &PathBuf, additional_args: &mut Vec<&str>) {
  let mut args = vec!["build", "-p", "membrane_tests"];
  args.append(additional_args);

  Command::new("cargo")
    .args(args)
    .output()
    .expect("lib could not be compiled for integration tests");

  // unlink the symlinks because they might be stale
  let _ = fs::remove_file(path.join("libmembrane_tests.so"));
  let _ = fs::remove_file(path.join("libmembrane_tests.dylib"));
  // link the workspace compiled artifacts to the temp test folder
  let _ = fs::hard_link(
    "../target/debug/libmembrane_tests.so",
    path.join("libmembrane_tests.so"),
  );
  let _ = fs::hard_link(
    "../target/debug/libmembrane_tests.dylib",
    path.join("libmembrane_tests.dylib"),
  );
}

pub fn run_dart(path: &PathBuf, args: Vec<&str>, verbose: bool) {
  let pub_get = Command::new("dart")
      .current_dir(&path)
      // set the library path to our temp pub project for linux
      .env("LD_LIBRARY_PATH", &path)
      .arg("--disable-analytics")
      .args(args)
      .output()
      .unwrap();

  if verbose {
    std::io::stdout().write_all(&pub_get.stdout).unwrap();
  }

  if pub_get.status.code() != Some(0) {
    if !verbose {
      std::io::stdout().write_all(&pub_get.stdout).unwrap();
    }
    std::io::stderr().write_all(&pub_get.stderr).unwrap();
    exit(1);
  }
}
