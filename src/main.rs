mod info_get;
mod key_get;
mod get_config;
mod login;
use std::process::exit;
use std::thread;
use std::time::Duration;
fn main() {
    let once = std::env::args().any(|a| a == "--once");
    let cfg = match get_config::load_config("/etc/zzu-wifi-login/config") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("fail to read config file : {}", e);
            eprintln!("tips:run sudo ./scripts/zzu-config-setup.sh");
            exit(1);
        },
    };
    
    let route = match info_get::route_to(&cfg.portal.to_string()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("web setup failed : {}", e);
            exit(1);
        }
    };
    let mac = match info_get::read_mac(&route.iface) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("can't read MAC : {}", e);
            exit(1);
        }
    };
    {
        let pass = match key_get::load_password() {
            Ok(o) => o,
            Err(e) => {
                println!("Can't load password");
                println!("Tips: run sudo ./scripts/zzu-cred-setup.sh to init or verify it");
                exit(1);
            }
        };
        println!("account  = {}", cfg.account);
        println!("carrier  = {}", cfg.carrier);
        println!("portal   = {}", cfg.portal);
        println!("ssid     = {}", cfg.ssid);
        println!("interval = {} 秒", cfg.interval_secs);
        println!("iface    = {}", route.iface);
        println!("src      = {}", route.src);
        println!("mac      = {}", mac);
        println!("password = {} 字节（不打印内容）", pass.len());
        let src = route.src.to_string();
        match login::login(&cfg, &pass, &src, &mac) {
            Ok(login::LoginResult::Ok) => println!("结果: 登录成功"),
            Ok(login::LoginResult::AlreadyOnline) => println!("结果: 已经在线，无需重复登录"),
            Ok(login::LoginResult::Failed(msg)) => println!("结果: 登录失败 - {}", msg),
            Err(e) => println!("结果: 请求失败 - {}", e),
        }
    }
    if once {
        exit(0);
    }
    let mut fails:u64 = 0;
    loop {
        let route = match info_get::route_to(&cfg.portal.to_string()) {
            Ok(o) => o,
            Err(e) => {
                fails += 1;
                println!("failed when rading net infomation: {},failed {} times",e,fails);
                thread::sleep(Duration::from_secs(cfg.interval_secs as u64));
                continue;
            }
        };
        let mac = match info_get::read_mac(&route.iface) {
            Ok(o) => o,
            Err(e) => {
                fails += 1;
                eprintln!("[{}]failed when rading mac infomation: {}",fails,e);
                thread::sleep(Duration::from_secs(cfg.interval_secs as u64));
                continue;
            }
        };
        let src = route.src.to_string();
        let online = match login::is_online(cfg.portal, &src) {
            Ok(v) => v,
            Err(e) => {
                fails += 1;
                eprintln!("[{}] reading network status failed : {}", fails, e);
                thread::sleep(Duration::from_secs(cfg.interval_secs as u64));
                continue;
            }
        };
        if online {
            println!("already online ip = {} mac = {}",src,mac);
            fails = 0;
        } else {
            let pass = match key_get::load_password() {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("Can't load password");
                    eprintln!("Tips: run sudo ./scripts/zzu-cred-setup.sh to init or verify it");
                    exit(1);
                }
            };
            println!("--- start login process (ip={} mac={}) ---", src, mac);
            match login::login(&cfg, &pass, &src, &mac) {
                Ok(login::LoginResult::Ok) => {
                    println!("login successful ");
                    fails = 0;
                }
                Ok(login::LoginResult::AlreadyOnline) => {
                    println!("already online");
                    fails = 0;
                }
                Ok(login::LoginResult::Failed(msg)) => {
                    fails += 1;
                    eprintln!("[{}] login failed - {}", fails, msg);
                }
                Err(e) => {
                    fails += 1;
                    eprintln!("[{}] request failed - {}", fails, e);
                }
            }
        }
        thread::sleep(Duration::from_secs(cfg.interval_secs as u64));
    }
}
