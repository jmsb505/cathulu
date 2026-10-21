use std::{env, fs, path::PathBuf};

const MAX_ENV_FILE_BYTES: u64 = 64 * 1024;
const MAX_API_KEY_BYTES: usize = 8 * 1024;

pub fn decision_api_key() -> Option<String> {
    read_key("CATHULU_DECISION_PROVIDER_API_KEY")
}

pub fn context_api_key() -> Option<String> {
    read_key("CATHULU_CONTEXT_PROVIDER_API_KEY")
}

fn read_key(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .and_then(|value| normalize_key(Some(value)))
        .or_else(|| find_env_file().and_then(|path| read_key_from_file(&path, name)))
}

fn find_env_file() -> Option<PathBuf> {
    if let Some(path) = env::var_os("CATHULU_ENV_FILE").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }

    env::current_dir()
        .ok()?
        .ancestors()
        .take(8)
        .map(|directory| directory.join(".env.local"))
        .find(|path| path.is_file())
}

fn read_key_from_file(path: &PathBuf, name: &str) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ENV_FILE_BYTES {
        return None;
    }
    let contents = fs::read_to_string(path).ok()?;
    read_key_from_contents(&contents, name)
}

fn read_key_from_contents(contents: &str, name: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == name {
            return normalize_key(Some(value.trim().to_string()));
        }
    }
    None
}

fn normalize_key(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    let unquoted = match (value.as_bytes().first(), value.as_bytes().last()) {
        (Some(b'"'), Some(b'"')) | (Some(b'\''), Some(b'\'')) if value.len() >= 2 => {
            &value[1..value.len() - 1]
        }
        (Some(b'"' | b'\''), _) | (_, Some(b'"' | b'\'')) => return None,
        _ => value,
    };
    if unquoted.is_empty()
        || unquoted.len() > MAX_API_KEY_BYTES
        || unquoted
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    Some(unquoted.to_string())
}

#[cfg(test)]
mod tests {
    use super::{normalize_key, read_key_from_contents};

    #[test]
    fn reads_only_named_nonempty_credentials_without_evaluating_shell_syntax() {
        let contents = "# local only\nOTHER_KEY=ignored\nCATHULU_DECISION_PROVIDER_API_KEY=jev-test-key\nCATHULU_CONTEXT_PROVIDER_API_KEY=\"scale-test-key\"\n";
        assert_eq!(
            read_key_from_contents(contents, "CATHULU_DECISION_PROVIDER_API_KEY").as_deref(),
            Some("jev-test-key")
        );
        assert_eq!(
            read_key_from_contents(contents, "CATHULU_CONTEXT_PROVIDER_API_KEY").as_deref(),
            Some("scale-test-key")
        );
        assert_eq!(
            read_key_from_contents(contents, "OTHER_KEY").as_deref(),
            Some("ignored")
        );
        assert_eq!(
            read_key_from_contents(
                "CATHULU_DECISION_PROVIDER_API_KEY=$(whoami)\n",
                "CATHULU_DECISION_PROVIDER_API_KEY"
            )
            .as_deref(),
            Some("$(whoami)")
        );
    }

    #[test]
    fn rejects_empty_malformed_or_multiline_credentials() {
        assert_eq!(normalize_key(Some("   ".to_string())), None);
        assert_eq!(normalize_key(Some("\"unclosed".to_string())), None);
        assert_eq!(normalize_key(Some("key with spaces".to_string())), None);
        assert_eq!(normalize_key(Some("line\nbreak".to_string())), None);
    }
}
