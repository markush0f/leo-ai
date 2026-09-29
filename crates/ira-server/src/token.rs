use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

pub fn load_http_token() -> Result<String, String> {
    if let Ok(raw) = std::env::var("IRA_HTTP_TOKEN") {
        let raw = raw.trim();
        if !raw.is_empty() {
            return Ok(raw.to_string());
        }
    }
    let path = ira_store::ira_home().join("http.token");
    if path.is_file() {
        let raw = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let raw = raw.trim();
        if !raw.is_empty() {
            return Ok(raw.to_string());
        }
    }
    create_token(&path)
}

fn create_token(path: &Path) -> Result<String, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(token.as_bytes())
                .and_then(|()| file.write_all(b"\n"))
                .map_err(|err| err.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|err| err.to_string())?;
            }
            Ok(token)
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let raw = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
            let raw = raw.trim();
            if raw.is_empty() {
                return Err(format!("token HTTP vacío en {}", path.display()));
            }
            Ok(raw.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}
