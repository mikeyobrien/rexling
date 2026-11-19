//! Which artwork a running program gets. Mirrors the Android client's `ProgramCatalog` and
//! `ProgramIdentity`; the alias table is copied from `ProgramCatalog.kt`.

const ALIASES: &[(&str, &str)] = &[
    ("agy", "antigravity"), ("antigravity-cli", "antigravity"), ("rex-server", "rex"), ("rex-cli", "rex"),
    ("cargo", "rust"), ("rustc", "rust"), ("kubectl", "kube"), ("helm", "kube"),
    ("k3s", "kube"), ("minikube", "kube"), ("gh", "github"), ("glab", "gitlab"),
    ("helix", "hx"), ("az", "azure"), ("psql", "postgres"), ("pgcli", "postgres"),
    ("sqlite3", "sqlite"), ("duck", "duckdb"), ("turbo", "turborepo"), ("mix", "elixir"),
    ("iex", "elixir"), ("kak", "kakoune"), ("jujutsu", "jj"), ("orb", "orbstack"),
    ("huggingface", "hugging_face"), ("hf", "hugging_face"), ("vp", "vite_plus"), ("batcat", "bat"),
    ("clang++", "clang"), ("cc", "clang"), ("c++", "clang"), ("docker-compose", "docker"),
    ("podman-compose", "podman"), ("container", "apple_container"), ("adb", "android"), ("homebrew", "brew"),
    ("brew.rb", "brew"), ("brew.sh", "brew"), ("emacsclient", "emacs"), ("xcodebuild", "xcode"),
    ("xcrun", "xcode"), ("jupyter-lab", "jupyter"), ("jupyter-notebook", "jupyter"), ("llama-cli", "llama_cpp"),
    ("llamacpp", "llama_cpp"), ("oh-my-posh", "omp"), ("poolside", "poolside_icon"), ("irb", "ruby"),
    ("gem", "ruby"), ("bundle", "ruby"), ("unreal", "unreal_engine"), ("buck2", "buck"),
    ("bazelisk", "bazel"), ("ibazel", "bazel"), ("gradlew", "gradle"), ("golang", "go"),
    ("nix-shell", "nix"), ("nixos-rebuild", "nix"), ("gh-copilot", "copilot"), ("github-copilot", "copilot"),
    ("nu", "nushell"),
];
const SHELLS: &[&str] = &["fish", "zsh", "bash", "sh", "dash", "ash", "ksh", "mksh", "tcsh", "csh", "nu", "nushell", "pwsh"];

fn executable(candidate: &str) -> String {
    let path = candidate.replace('\\', "/").to_lowercase();
    let name = path.rsplit('/').next().unwrap_or("").trim_start_matches('-');
    name.trim_end_matches(".exe").trim_end_matches(".cmd").to_owned()
}

/// Names that identify a program regardless of the tile catalogue.
fn known(name: &str, path: &str) -> Option<&'static str> {
    let python = name == "python" || name == "ipython" || name.strip_prefix("python").is_some_and(|v| v.starts_with(['2', '3']));
    Some(match name {
        "claude" | "claude-code" | "claude-code.js" => "claude_code",
        "codex" | "codex.js" => "codex",
        "nvim" | "neovim" => "neovim",
        "vim" | "vi" | "gvim" => "vim",
        "git" | "lazygit" | "tig" => "git",
        "node" | "nodejs" | "node.js" | "npm" | "yarn" | "deno" => "node",
        "htop" => "htop",
        "btop" => "btop",
        "top" => "top",
        "cloudflared" | "cloudflare" => "cloudflare",
        _ if python => "python",
        _ if path.contains("/@anthropic-ai/claude-code/") => "claude_code",
        _ if path.contains("/@openai/codex/") || name.starts_with("codex-") => "codex",
        _ => return None,
    })
}

/// The tile for a pane, given the programs running in it (outermost first), and whether a tile with that name exists.
pub fn tile(candidates: &[String], exists: impl Fn(&str) -> bool) -> String {
    let mut runtime = None;
    let mut shell = None;
    for candidate in candidates {
        let name = executable(candidate);
        let path = candidate.replace('\\', "/").to_lowercase();
        if let Some(tile) = known(&name, &path) {
            // An interpreter is only the answer when nothing more specific runs inside it.
            if matches!(tile, "node" | "python") {
                runtime.get_or_insert(tile.to_owned());
                continue;
            }
            return tile.to_owned();
        }
        if SHELLS.contains(&name.as_str()) {
            shell.get_or_insert(name);
            continue;
        }
        let aliased = ALIASES.iter().find(|(from, _)| *from == name).map(|(_, to)| (*to).to_owned());
        let tile = aliased.unwrap_or_else(|| name.replace('-', "_"));
        if !matches!(tile.as_str(), "shell" | "terminal" | "placeholder" | "minimal") && exists(&tile) {
            return tile;
        }
    }
    runtime
        .or_else(|| shell.map(|name| if name == "nu" { "nushell".to_owned() } else { name }).filter(|name| exists(name)))
        .unwrap_or_else(|| "shell".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(programs: &[&str]) -> String {
        let programs: Vec<String> = programs.iter().map(|p| (*p).to_owned()).collect();
        tile(&programs, |name| ["bash", "zsh", "docker", "rust", "kube", "shell"].contains(&name))
    }

    #[test]
    fn picks_the_most_specific_program() {
        assert_eq!(resolve(&["-bash", "/usr/bin/bash", "bash"]), "bash");
        assert_eq!(resolve(&["-zsh", "/usr/bin/node", "/home/u/.npm/@anthropic-ai/claude-code/cli.js"]), "claude_code");
        assert_eq!(resolve(&["bash", "/usr/bin/python3.12"]), "python");
        assert_eq!(resolve(&["bash", "nvim"]), "neovim");
        assert_eq!(resolve(&["bash", "cargo"]), "rust");
        assert_eq!(resolve(&["bash", "kubectl"]), "kube");
        assert_eq!(resolve(&["dash"]), "shell");
        assert_eq!(resolve(&[]), "shell");
    }
}
