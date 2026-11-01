use std::fs;
use std::net::Ipv4Addr;
use std::str;
pub struct Config {
    pub account : String,
    pub carrier : String,
    pub ssid : String,
    pub interval_secs : i32,
    pub portal : Ipv4Addr,
}
pub fn load_config(path : &str) -> Result<Config,String>
{
    let fcon = match fs::read_to_string(path) {
        Ok(o) => o,
        Err(e) => return Err(format!("read file {} failed : {}",path,e)),
    };
    // let sli : Vec<&str> = fcon.split('\n').collect();
    let mut loaded : Config = Config{account : String::new(),carrier : String::new(),ssid : String::from("ZZU-WLAN"),interval_secs : 60,portal : Ipv4Addr::new(172,16,4,14)};

    for i in fcon.lines() {
        if (*i).starts_with("account=") {
            loaded.account = match (*i).strip_prefix("account=") {
                Some(o) => o.to_string(),
                None => return Err(String::from("account is empty"))
            };
        } else if (*i).starts_with("carrier=") {
            loaded.carrier = match (*i).strip_prefix("carrier=") {
                Some(o) => o.to_string(),
                None => String::new(),
            };
        } else if (*i).starts_with("ssid=") {
            loaded.ssid = match (*i).strip_prefix("ssid=") {
                Some(o) => o.to_string(),
                None => return Err(String::from("ssid is empty"))
            };
        } else if (*i).starts_with("interval_secs=") {
            let text = match (*i).strip_prefix("interval_secs=") {
                Some(o) => o,
                None => return Err(String::from("interval_secs is empty")),
            };
            loaded.interval_secs = match text.parse() {
                Ok(n) => n,
                Err(_) => return Err(format!("interval_secs is not a number : {}", text)),
            };
        } else if (*i).starts_with("portal=") {
            let text = match (*i).strip_prefix("portal=") {
                Some(o) => o,
                None => return Err(String::from("portal is empty")),
            };
            loaded.portal = match text.parse() {
                Ok(ip) => ip,
                Err(_) => return Err(format!("invaled portal IPv4: {}", text)),
            };
        }
    }
    if loaded.account == "" {
        return Err("invaled account or broken configer".to_string());
    }
    Ok(loaded)
}
#[cfg(test)]
mod tests {
    use super::*;

    // 临时文件路径：带进程号，避免并行测试互相踩
    fn tmp(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("zzu_cfg_test_{}_{}", std::process::id(), name));
        p.display().to_string()
    }

    #[test]
    fn parses_full_config() {
        let path = tmp("full");
        let text = "\
# 由脚本生成的配置
account=202600000001
carrier=cmcc
portal=172.16.4.14
ssid=ZZU-WLAN
interval_secs=45
";
        std::fs::write(&path, text).unwrap();

        let cfg = load_config(&path).unwrap();

        assert_eq!(cfg.account, "202600000001");
        assert_eq!(cfg.carrier, "cmcc");
        assert_eq!(cfg.portal, Ipv4Addr::new(172, 16, 4, 14));
        assert_eq!(cfg.ssid, "ZZU-WLAN");
        assert_eq!(cfg.interval_secs, 45);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn uses_defaults() {
        let path = tmp("defaults");
        std::fs::write(&path, "account=123456\n").unwrap();

        let cfg = load_config(&path).unwrap();

        assert_eq!(cfg.ssid, "ZZU-WLAN");
        assert_eq!(cfg.interval_secs, 60);
        assert_eq!(cfg.portal, Ipv4Addr::new(172, 16, 4, 14));
        assert_eq!(cfg.carrier, "");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_bad_interval() {
        let path = tmp("bad_interval");
        std::fs::write(&path, "account=1\ninterval_secs=abc\n").unwrap();

        let r = load_config(&path);
        assert!(r.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_bad_portal() {
        let path = tmp("bad_portal");
        std::fs::write(&path, "account=1\nportal=999.1.1.1\n").unwrap();

        let r = load_config(&path);
        assert!(r.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_account() {
        let path = tmp("empty_account");
        std::fs::write(&path, "account=\nportal=1.1.1.1\n").unwrap();

        let r = load_config(&path);
        assert!(r.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_missing_file() {
        let r = load_config("/nonexistent/zzu-config-test");
        assert!(r.is_err());
    }

    #[test]
    fn handles_crlf() {
        let path = tmp("crlf");
        std::fs::write(&path, "account=1\r\nportal=1.1.1.1\r\nssid=ABC\r\n").unwrap();

        let cfg = load_config(&path).unwrap();

        assert_eq!(cfg.ssid, "ABC");

        let _ = std::fs::remove_file(&path);
    }
}
