use std::fs;
use std::net::Ipv4Addr;
use std::process::Command;
#[derive(Debug)]
pub struct Route {
    pub iface: String,
    pub src: Ipv4Addr,
    pub gateway: Option<Ipv4Addr>,
}
// fn value_after(tokens: &[&str], key: &str) -> Option<String> {
//     let mut i = 0;
//     while i < tokens.len() {
//         if tokens[i] == key && i + 1 < tokens.len() {
//             return Some(tokens[i + 1].to_string());
//         }
//         i += 1;
//     }
//     None
// }
fn value_after(sli : &[&str],key : &str) -> Option<String>
{
    let mut vis : bool = false;
    for i in sli {
        if vis == true {
            return Some((*i).to_string());
        }
        if *i == key {
            vis = true;
        }
    }
    None
}
fn parse_route(addr : &str) -> Option<Route>
{
    let sli : Vec<&str> = addr.split_whitespace().collect();
    let ipface = match value_after(&sli,"dev") {
        Some(v) => v,
        None => return None,
    };
    let src_text = match value_after(&sli,"src") {
        Some(v) => v,
        None => return None,
    };
    let src0 : Ipv4Addr = match src_text.parse() {
        Ok(v) => v,
        Err(_) => return None,
    };
    // let mut gateway : Option<Ipv4Addr> = None;
    // if let Some(text) = value_after(&sli,"via") {
    //     if let Ok(ip) = text.parse() {
    //         gateway = some(ip);
    //     }
    // }
    let gateway : Option<Ipv4Addr> = value_after(&sli,"via").and_then(|text| text.parse().ok());//and_then helps doing following things after you get the value,.ok() transfer Result to Option the avoid panic
    Some(Route {iface : ipface,src : src0,gateway})
}
pub fn route_to(dest : &str) -> Result<Route,String>
{
    let out = Command::new("ip").args(["route","get",dest]).output();
    let out  = match out{
        Ok(o) => o,
        Err(e) => return Err(format!("无法执行 ip 命令: {}", e)),
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ip route get {} failed {}",dest,err.trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    match parse_route(&text) {
        Some(r) => Ok(r),
        None => Err(format!("ip formate error : {}",text.trim())),
    }
}
pub fn read_mac(iface : &str) -> Result<String,String>
{
    let path = format!("/sys/class/net/{}/address", iface);
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => return Err(format!("read {} failed : {}",path,e)),
    };
    let mac = raw.trim().replace(":","").to_lowercase();
    if mac.len() != 12 {
        return Err(format!("{} is not like MAC addr : {:?}", path,raw.trim()));
    }
    Ok(mac)
}
