use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::get_config::Config;

const AC_IP: &str = "172.16.4.252";
const API_PORT: u16 = 801;
const JS_VERSION: &str = "4.2.2";

pub enum LoginResult {
    Ok,
    AlreadyOnline,
    Failed(String),
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(B64[((n >> 6) & 63) as usize] as char);
        out.push(B64[(n & 63) as usize] as char);
        i += 3;
    }
    let left = input.len() - i;
    if left == 1 {
        let n = (input[i] as u32) << 16;
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if left == 2 {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(B64[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn encode_for_url(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '+' => out.push_str("%2B"),
            '/' => out.push_str("%2F"),
            '=' => out.push_str("%3D"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn build_login_path(cfg: &Config, pass: &str, ip: &str, mac: &str, token: u32) -> String {
    let suffix = if cfg.carrier.is_empty() {
        String::new()
    } else {
        format!("@{}", cfg.carrier)
    };
    let account = format!(",0,{}{}", cfg.account, suffix);
    let pass_b64 = encode_for_url(&base64_encode(pass.as_bytes()));
    format!(
        "/eportal/portal/login?callback=dr1003&login_method=1&user_account={}&user_password={}\
&wlan_user_ip={}&wlan_user_ipv6=&wlan_user_mac={}&wlan_vlan_id=1&wlan_ac_ip={}\
&wlan_ac_name=&authex_enable=&jsVersion={}&terminal_type=1&lang=zh\
&program_index=&page_index=&v={}",
        account, pass_b64, ip, mac, AC_IP, JS_VERSION, token
    )
}

fn json_field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = match body.find(&needle) {
        Some(i) => i + needle.len(),
        None => return None,
    };
    let mut out = String::new();
    for ch in body[start..].chars() {
        if ch == ',' || ch == '}' {
            break;
        }
        if ch != '"' {
            out.push(ch);
        }
    }
    let text = out.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub fn parse_reply(body: &str) -> LoginResult {
    if let Some(v) = json_field(body, "result") {
        if v == "1" || v == "ok" {
            return LoginResult::Ok;
        }
    }
    if let Some(v) = json_field(body, "ret_code") {
        if v == "2" {
            return LoginResult::AlreadyOnline;
        }
    }
    let msg = match json_field(body, "msg") {
        Some(m) => m,
        None => match json_field(body, "msga") {
            Some(m) => m,
            None => String::from("unrecognized response"),
        },
    };
    if msg.contains("已经在线") || msg.contains("已在线") {
        return LoginResult::AlreadyOnline;
    }
    if body.trim().is_empty() {
        return LoginResult::Failed(String::from("empty response"));
    }
    LoginResult::Failed(msg)
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 3 < raw.len() {
        if raw[i] == 13 && raw[i + 1] == 10 && raw[i + 2] == 13 && raw[i + 3] == 10 {
            return Some(i + 4);
        }
        i += 1;
    }
    None
}

fn content_length(head: &[u8]) -> Option<usize> {
    let text = String::from_utf8_lossy(head).to_string();
    for line in text.lines() {
        let low = line.to_lowercase();
        if low.starts_with("content-length:") {
            let value = match line.split(':').nth(1) {
                Some(v) => v.trim(),
                None => return None,
            };
            return value.parse::<usize>().ok();
        }
    }
    None
}

fn http_get(ip: Ipv4Addr, port: u16, path: &str, timeout_secs: u64) -> Result<String, String> {
    let addr = SocketAddr::new(std::net::IpAddr::V4(ip), port);
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(timeout_secs)) {
        Ok(s) => s,
        Err(e) => return Err(format!("connect {}:{} failed : {}", ip, port, e)),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(timeout_secs)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(timeout_secs)));

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: zzu-wifi-login/0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, ip
    );
    if let Err(e) = stream.write_all(req.as_bytes()) {
        return Err(format!("send request failed : {}", e));
    }

    let mut raw: Vec<u8> = Vec::new();
    let mut buf = [0u8; 1024];

    // 阶段一：读到空行为止（响应头结束）
    let mut header_end = 0;
    loop {
        if let Some(pos) = find_header_end(&raw) {
            header_end = pos;
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(e) => return Err(format!("read header failed : {}", e)),
        }
    }
    if header_end == 0 {
        return Err(String::from("malformed http response"));
    }

    // 阶段二：有 Content-Length 就按长度读完，没有就读到对端关闭/超时
    match content_length(&raw[..header_end]) {
        Some(want) => {
            while raw.len() < header_end + want {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => raw.extend_from_slice(&buf[..n]),
                    Err(_) => break,
                }
            }
        }
        None => loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw.extend_from_slice(&buf[..n]);
                }
                Err(_) => break,
            }
        },
    }

    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let first = match head.lines().next() {
        Some(l) => l,
        None => return Err(String::from("malformed http response")),
    };
    if !first.contains(" 200") {
        return Err(format!("http status : {}", first));
    }

    Ok(String::from_utf8_lossy(&raw[header_end..]).to_string())
}

pub fn is_online(portal: Ipv4Addr, client_ip: &str) -> Result<bool, String> {
    let path = format!(
        "/drcom/chkstatus?callback=dr1002&v4ip={}&jsVersion={}&v=1234",
        client_ip, JS_VERSION
    );
    let body = http_get(portal, 80, &path, 5)?;
    match json_field(&body, "result") {
        Some(v) => Ok(v == "1"),
        None => Err(format!("状态响应无法解析: {}", &body[..body.len().min(120)])),
    }
}

pub fn login(cfg: &Config, pass: &str, ip: &str, mac: &str) -> Result<LoginResult, String> {
    let token = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_millis() % 10000) as u32)
        .unwrap_or(1000);
    let path = build_login_path(cfg, pass, ip, mac, token);
    let body = http_get(cfg.portal, API_PORT, &path, 10)?;
    Ok(parse_reply(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_config::Config;

    fn cfg() -> Config {
        Config {
            account: String::from("202600000001"),
            carrier: String::from("cmcc"),
            ssid: String::from("ZZU-WLAN"),
            interval_secs: 30,
            portal: Ipv4Addr::new(172, 16, 4, 14),
        }
    }

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64_encode(b"123"), "MTIz");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abcd"), "YWJjZA==");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn url_encodes_base64_specials() {
        assert_eq!(encode_for_url("a+b/c="), "a%2Bb%2Fc%3D");
    }

    #[test]
    fn builds_account_with_suffix() {
        let path = build_login_path(&cfg(), "x", "10.0.0.2", "aabbccddeeff", 1);
        assert!(path.contains("user_account=,0,202600000001@cmcc"));
        assert!(path.contains("wlan_user_ip=10.0.0.2"));
        assert!(path.contains("wlan_user_mac=aabbccddeeff"));
        assert!(path.contains("jsVersion=4.2.2"));
        assert!(path.starts_with("/eportal/portal/login?"));
    }

    #[test]
    fn parses_success() {
        let r = parse_reply(r#"dr1003({"result":1,"msg":"认证成功"})"#);
        assert!(matches!(r, LoginResult::Ok));
    }

    #[test]
    fn parses_already_online() {
        let r = parse_reply(r#"dr1003({"result":0,"ret_code":2,"msg":"已经在线"})"#);
        assert!(matches!(r, LoginResult::AlreadyOnline));
    }

    #[test]
    fn parses_failure() {
        let r = parse_reply(r#"dr1003({"result":0,"msg":"密码错误"})"#);
        match r {
            LoginResult::Failed(m) => assert_eq!(m, "密码错误"),
            _ => panic!("should fail"),
        }
    }
}