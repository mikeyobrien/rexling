//! `ssh://user@host` servers. The system's `ssh` forwards the remote Rex socket to a local one,
//! so its keys, agent, config and known hosts all apply, and Rexling stores no credentials.
use anyhow::{Context, Result, bail, ensure};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Tunnel {
    child: Child,
    socket: PathBuf,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// The Rex socket of the account logged in to this computer, if a server is listening there.
pub fn local_server() -> Option<String> {
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("rex/server.sock");
    std::os::unix::net::UnixStream::connect(&socket).ok().map(|_| format!("unix://{}", socket.display()))
}

struct Target {
    destination: String,
    port: Option<String>,
}

fn target(server: &str) -> Result<Target> {
    let rest = server.strip_prefix("ssh://").context("not an ssh:// address")?.trim_end_matches('/');
    let (user, host) = rest.split_once('@').context("use ssh://user@host")?;
    let (host, port) = match host.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => (host, Some(port.to_owned())),
        _ => (host, None),
    };
    let plain = |text: &str| !text.is_empty() && !text.starts_with('-') && text.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    ensure!(plain(user) && plain(host), "use ssh://user@host with a plain user and host name");
    Ok(Target { destination: format!("{user}@{host}"), port })
}

fn ssh(target: &Target) -> Command {
    let mut command = Command::new("ssh");
    // Never prompt: there is no terminal to answer on. Keys or an agent must already work.
    command.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]);
    if let Some(port) = &target.port {
        command.args(["-p", port]);
    }
    command.stdin(Stdio::null());
    command
}

/// Opens the tunnel and returns it with the `unix://` address to connect to. Blocks; call off the main thread.
pub fn open(server: &str) -> Result<(Tunnel, String)> {
    let target = target(server)?;
    let output = ssh(&target)
        .arg(&target.destination)
        .arg(r#"printf %s "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/rex/server.sock""#)
        .output()
        .context("could not run ssh")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).lines().last().unwrap_or("ssh could not connect").trim().to_owned());
    }
    let remote = String::from_utf8(output.stdout).context("unexpected reply from the remote shell")?;
    ensure!(remote.starts_with('/') && !remote.contains('\n'), "the remote account has no Rex socket path");

    let directory = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?).join("rexling");
    std::fs::create_dir_all(&directory)?;
    let name: String = target.destination.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let socket = directory.join(format!("ssh-{name}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket);
    let child = ssh(&target)
        .args(["-o", "ExitOnForwardFailure=yes", "-o", "ServerAliveInterval=15", "-o", "StreamLocalBindUnlink=yes", "-N", "-L"])
        .arg(format!("{}:{remote}", socket.display()))
        .arg(&target.destination)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not run ssh")?;
    let mut tunnel = Tunnel { child, socket: socket.clone() };
    let deadline = Instant::now() + Duration::from_secs(12);
    while !socket.exists() {
        if let Some(status) = tunnel.child.try_wait()? {
            let mut message = String::new();
            if let Some(mut stderr) = tunnel.child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut stderr, &mut message);
            }
            bail!("{}", message.lines().last().map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned).unwrap_or_else(|| format!("ssh exited ({status})")));
        }
        ensure!(Instant::now() < deadline, "the SSH tunnel did not come up");
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok((tunnel, format!("unix://{}", socket.display())))
}

/// A short name for a server address: the host, or "This Computer" for the local socket.
pub fn label(server: &str) -> String {
    if let Some(path) = server.strip_prefix("unix://") {
        // The account's own socket is this computer; any other socket is known by its folder.
        if path.ends_with("/rex/server.sock") {
            return "This Computer".into();
        }
        let folder = std::path::Path::new(path).parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        return folder.unwrap_or_else(|| "Local Socket".into());
    }
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest).trim_end_matches('/');
    let host = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
    let host = host.split(':').next().unwrap_or(host);
    host.split('.').next().unwrap_or(host).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_addresses() {
        let plain = target("ssh://ada@build.example.org").unwrap();
        assert_eq!((plain.destination.as_str(), plain.port), ("ada@build.example.org", None));
        let ported = target("ssh://ada@build:2222/").unwrap();
        assert_eq!((ported.destination.as_str(), ported.port.as_deref()), ("ada@build", Some("2222")));
        assert!(target("ssh://build").is_err(), "a user is required");
        assert!(target("ssh://-oProxyCommand=x@host").is_err(), "options cannot be smuggled in as a user");
        assert!(target("ssh://ada@ho st").is_err());
    }

    #[test]
    fn names_servers_briefly() {
        assert_eq!(label("https://columba.example.ts.net:7297"), "columba");
        assert_eq!(label("ssh://ada@build.example.org"), "build");
        assert_eq!(label("unix:///run/user/1000/rex/server.sock"), "This Computer");
        assert_eq!(label("unix:///srv/staging/server.sock"), "staging");
    }
}
