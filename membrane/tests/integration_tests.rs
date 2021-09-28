use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| Builder::new_multi_thread().build().unwrap());

mod test_app {
  use membrane::async_dart;
  use serde::{Deserialize, Serialize};

  #[derive(Serialize, Deserialize)]
  pub struct User {
    id: i64,
    full_name: String,
  }

  #[async_dart(namespace = "users")]
  pub async fn get_user(user_id: i64) -> Result<User, String> {
    Ok(User {
      id: user_id,
      full_name: "Test User".to_string(),
    })
  }
}

mod test {
  use membrane::Membrane;
  use std::fs::read_to_string;
  use tempfile::tempdir;

  #[test]
  fn base_project() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_project");

    Membrane::new()
      .package_destination_dir(path.to_str().unwrap())
      .using_lib("libtest")
      .create_pub_package()
      .write_api()
      .write_c_headers();

    let api = read_to_string(path.join("lib").join("users.dart")).unwrap();
    assert!(api.contains("@immutable\nclass UsersApi {"));
    assert!(api.contains("Future<User> getUser({required int userId}) async {"));
    let dart_type =
      read_to_string(path.join("lib").join("src").join("users").join("user.dart")).unwrap();
    assert!(dart_type
      .split_whitespace()
      .collect::<String>()
      .contains::<&str>(
        &r#"@immutable
class User {
  const User({
    required this.id,
    required this.fullName,
  });"#
          .split_whitespace()
          .collect::<String>()
      ));

    let headers =
      read_to_string(path.join("lib").join("src").join("users").join("users.h")).unwrap();

    assert!(
      headers.contains("int32_t membrane_users_get_user(int64_t port, const signed long user_id);")
    );
  }
}
