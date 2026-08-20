use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_VTEST_SOCKET: &str = "/tmp/.virgl_test";
pub const DEFAULT_GUEST_SOCKET: &str = "/run/xdg/.virgl_test";
pub const DEFAULT_VENUS_ICD: &str = "/vendor/etc/vulkan/icd.d/virtio_icd.x86_64.json";
pub const DEFAULT_WAYDROID_ROOT: &str = "/var/lib/waydroid";

#[derive(Debug, Clone)]
pub struct WaydroidVenusConfig {
    pub server: PathBuf,
    pub socket_host: PathBuf,
    pub socket_guest: PathBuf,
    pub render_node: Option<PathBuf>,
    pub host_vulkan_icd: Option<PathBuf>,
    pub vulkan_icd: Option<PathBuf>,
    pub config_session: PathBuf,
    pub init_environ_rc: Option<PathBuf>,
    pub waydroid_props: Vec<PathBuf>,
}

impl Default for WaydroidVenusConfig {
    fn default() -> Self {
        let root = PathBuf::from(DEFAULT_WAYDROID_ROOT);
        Self {
            server: PathBuf::from("virgl_test_server"),
            socket_host: PathBuf::from(DEFAULT_VTEST_SOCKET),
            socket_guest: PathBuf::from(DEFAULT_GUEST_SOCKET),
            render_node: None,
            host_vulkan_icd: None,
            vulkan_icd: Some(PathBuf::from(DEFAULT_VENUS_ICD)),
            config_session: root.join("config_session"),
            init_environ_rc: None,
            waydroid_props: vec![root.join("waydroid.prop")],
        }
    }
}

impl WaydroidVenusConfig {
    pub fn guest_env_lines(&self) -> Vec<String> {
        let icd = self
            .vulkan_icd
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_VENUS_ICD));
        vec![
            "export VN_DEBUG vtest".to_owned(),
            format!("export VTEST_SOCKET_NAME {}", self.socket_guest.display()),
            format!("export VK_DRIVER_FILES {}", icd.display()),
            "export GALLIUM_DRIVER virpipe".to_owned(),
            "export LIBVA_DRIVER_NAME virtio_gpu".to_owned(),
        ]
    }

    pub const fn prop_lines() -> &'static [&'static str] {
        &[
            "ro.hardware.vulkan=virtio",
            "ro.hardware.egl=mesa",
            "ro.hardware.gralloc=gbm",
        ]
    }

    fn discovered_init_environ(&self) -> Option<PathBuf> {
        if let Some(path) = &self.init_environ_rc {
            return Some(path.clone());
        }
        let root = Path::new(DEFAULT_WAYDROID_ROOT);
        [
            root.join("overlay/init.environ.rc"),
            root.join("overlay/system/etc/init.environ.rc"),
            root.join("rootfs/init.environ.rc"),
        ]
        .into_iter()
        .find(|path| path.is_file())
    }

    pub fn patch_config_session(&self) -> io::Result<bool> {
        let guest_socket = self
            .socket_guest
            .strip_prefix(Path::new("/"))
            .unwrap_or(&self.socket_guest);
        let entry = format!(
            "lxc.mount.entry = {} {} none bind,create=file,optional 0 0",
            self.socket_host.display(),
            guest_socket.display()
        );
        ensure_line(&self.config_session, &entry)
    }

    pub fn patch_init_environ(&self) -> io::Result<bool> {
        let Some(path) = self.discovered_init_environ() else {
            return Ok(false);
        };
        let mut content = fs::read_to_string(&path)?;
        let mut changed = false;
        if !content.contains("on init") {
            content.push_str("\non init\n");
            changed = true;
        }
        for line in self.guest_env_lines() {
            if !content.lines().any(|existing| existing.trim() == line) {
                content.push_str(&format!("    {}\n", line));
                changed = true;
            }
        }
        if changed {
            fs::write(&path, content)?;
        }
        Ok(changed)
    }

    pub fn patch_props(&self) -> io::Result<usize> {
        let mut changed = 0;
        for path in &self.waydroid_props {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut content = if path.exists() {
                fs::read_to_string(path)?
            } else {
                String::new()
            };
            let original = content.clone();
            for prop in Self::prop_lines() {
                let key = prop.split('=').next().unwrap_or_default();
                let mut found = false;
                let mut lines = Vec::new();
                for line in content.lines() {
                    if line.starts_with(&format!("{}=", key)) {
                        if !found {
                            lines.push(*prop);
                            found = true;
                        }
                    } else {
                        lines.push(line);
                    }
                }
                if !found {
                    lines.push(*prop);
                }
                content = lines.join("\n");
                if !content.ends_with('\n') {
                    content.push('\n');
                }
            }
            if content != original {
                fs::write(path, content)?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    pub fn spawn_server(&self) -> io::Result<Child> {
        if self.socket_host.exists() {
            let _ = fs::remove_file(&self.socket_host);
        }

        let mut cmd = Command::new(&self.server);
        cmd.arg("--venus")
            .arg("--no-fork")
            .arg("--multi-clients")
            .arg("--use-egl-surfaceless")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if let Some(render_node) = &self.render_node {
            cmd.env("VTEST_RENDERNODE", render_node);
        }
        if let Some(host_icd) = &self.host_vulkan_icd {
            cmd.env("VK_DRIVER_FILES", host_icd);
        }
        let child = cmd.spawn()?;
        wait_for_path(&self.socket_host, Duration::from_secs(5))?;
        set_socket_mode(&self.socket_host, 0o666)?;
        Ok(child)
    }

    pub fn setup(&self) -> io::Result<()> {
        self.patch_config_session()?;
        self.patch_props()?;
        self.patch_init_environ()?;
        Ok(())
    }
}

fn ensure_line(path: &Path, line: &str) -> io::Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if content
        .lines()
        .any(|existing| existing.trim() == line.trim())
    {
        return Ok(false);
    }
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(line);
    content.push('\n');
    fs::write(path, content)?;
    Ok(true)
}

fn wait_for_path(path: &Path, timeout: Duration) -> io::Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("timed out waiting for {}", path.display()),
    ))
}

fn set_socket_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-virtio-gpu-{name}-{nonce}"))
    }

    #[test]
    fn config_session_patch_is_idempotent() {
        let path = temp_file("config");
        fs::write(&path, "lxc.arch = linux\n").unwrap();
        let config = WaydroidVenusConfig {
            config_session: path.clone(),
            ..Default::default()
        };
        assert!(config.patch_config_session().unwrap());
        assert!(!config.patch_config_session().unwrap());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(
            content
                .lines()
                .filter(|line| line.starts_with("lxc.mount.entry = /tmp/.virgl_test "))
                .count(),
            1
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn init_env_contains_vulkan_and_vtest() {
        let config = WaydroidVenusConfig::default();
        let env = config.guest_env_lines();
        assert!(env.iter().any(|line| line == "export VN_DEBUG vtest"));
        assert!(
            env.iter()
                .any(|line| line.starts_with("export VTEST_SOCKET_NAME "))
        );
        assert!(
            env.iter()
                .any(|line| line.starts_with("export VK_DRIVER_FILES "))
        );
    }

    #[test]
    fn default_setup_targets_waydroid_prop() {
        let config = WaydroidVenusConfig::default();
        assert_eq!(config.waydroid_props.len(), 1);
        assert!(config.waydroid_props[0].ends_with("waydroid.prop"));
    }

    #[test]
    fn props_are_stable() {
        assert_eq!(WaydroidVenusConfig::prop_lines().len(), 3);
    }
}
