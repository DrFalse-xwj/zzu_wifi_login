use std::env;
use std::fs;
const CRED_NAME : &str = "zzu-pass";
fn read_file(path : &str) -> Result<String,String>
{
    let raw = match fs::read_to_string(path) {
        Ok(o) => o,
        Err(e) => return Err(format!("read {} failed : {}",path,e)),
    };
    let pass = raw.trim();
    if pass.is_empty() {
        return Err(format!("password in {} is empty",path));
    }
    Ok(pass.to_string())
}
pub fn load_password() -> Result<String,String>
{
    if let Ok(dir) = env::var("CREDENTIALS_DIRECTORY") {
        return read_file(&format!("{}/{}",dir,CRED_NAME));
    }
    match env::var("ZZU_PASS") {
        Ok(p) => return Ok(p.trim().to_string()),
        Err(_) => return Err(String::from("testing key not loaded")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        let mut p = env::temp_dir();
        p.push(format!("zzu_test_{}_{}", std::process::id(), name));
        p.display().to_string()
    }

    #[test]
    fn reads_and_trims() {
        let path = tmp("trim");
        fs::write(&path, "  abc \n\n").unwrap();
        assert_eq!(read_file(&path).unwrap(), "abc");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_file() {
        let path = tmp("empty");
        fs::write(&path, "   \n").unwrap();
        assert!(read_file(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_missing_file() {
        assert!(read_file("/nonexistent/zzu-test").is_err());
    }
}
