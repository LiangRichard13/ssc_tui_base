use super::*;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

#[test]
fn jcode_dir_defaults_to_saitec_home_directory() {
    let _home = EnvVarGuard::unset("JCODE_HOME");

    let home = dirs::home_dir().expect("home dir");
    let actual = jcode_dir().expect("jcode dir");

    assert_eq!(actual, home.join(".jcode"));
}

#[test]
fn app_config_dir_is_sandboxed_under_saitec_home_when_jcode_home_is_set() {
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let actual = app_config_dir().expect("config dir");

    assert_eq!(actual, temp.path().join("config").join("jcode"));
}
