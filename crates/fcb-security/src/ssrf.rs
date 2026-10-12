use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::sync::LazyLock;
use fcb_core::AppError;
use ipnet::{Ipv4Net, Ipv6Net};
use url::Url;

pub const ENDPOINT_HOST_DENY_SUFFIXES: &[&str] = &[
    ".local",
    ".internal",
    ".lan",
    ".home.arpa",
    ".corp",
    ".localhost",
];

static DENIED_IPV4_NETS: LazyLock<Vec<Ipv4Net>> = LazyLock::new(|| {
    let cidrs = [
        "0.0.0.0/8",       // 当前网络 ("this host")
        "10.0.0.0/8",      // 私有网络
        "100.64.0.0/10",   // 运营商级 NAT (Shared Address Space)
        "127.0.0.0/8",     // 环回地址
        "169.254.0.0/16",  // 链路本地 (Link-Local)
        "172.16.0.0/12",   // 私有网络
        "192.0.0.0/24",    // IETF 协议分配
        "192.0.2.0/24",    // TEST-NET-1
        "192.88.99.0/24",  // 6to4 任播中继
        "192.168.0.0/16",  // 私有网络
        "198.18.0.0/15",   // 网络基准测试
        "198.51.100.0/24", // TEST-NET-2
        "203.0.113.0/24",  // TEST-NET-3
        "224.0.0.0/4",     // 多播
        "240.0.0.0/4",     // 保留地址 (包含广播 255.255.255.255)
    ];
    cidrs
        .iter()
        .map(|s| s.parse::<Ipv4Net>().expect("Valid IPv4 CIDR"))
        .collect()
});

static DENIED_IPV6_NETS: LazyLock<Vec<Ipv6Net>> = LazyLock::new(|| {
    let cidrs = [
        "::/128",          // 未指定地址
        "::1/128",         // 环回地址
        "::/96",           // IPv4 兼容地址
        "100::/64",        // 丢弃前缀
        "2001:db8::/32",   // 文档地址
        "fc00::/7",        // 唯一本地地址 (ULA)
        "fe80::/10",       // 链路本地单播
        "ff00::/8",        // 多播
    ];
    cidrs
        .iter()
        .map(|s| s.parse::<Ipv6Net>().expect("Valid IPv6 CIDR"))
        .collect()
});

/// 检查 IP 是否为内网/回环/保留/多播/未指定地址
pub fn is_ip_denied(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => DENIED_IPV4_NETS.iter().any(|net| net.contains(ipv4)),
        IpAddr::V6(ipv6) => {
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) 或 IPv4-compatible (::x.x.x.x)
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_ip_denied(&IpAddr::V4(ipv4));
            }
            let segs = ipv6.segments();
            if segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 && segs[4] == 0 && segs[5] == 0xffff {
                let v4 = Ipv4Addr::new(
                    (segs[6] >> 8) as u8,
                    (segs[6] & 0xff) as u8,
                    (segs[7] >> 8) as u8,
                    (segs[7] & 0xff) as u8,
                );
                return is_ip_denied(&IpAddr::V4(v4));
            }
            if let Some(ipv4) = ipv6.to_ipv4() {
                if !ipv6.is_unspecified() && !ipv6.is_loopback() {
                    return is_ip_denied(&IpAddr::V4(ipv4));
                }
            }
            DENIED_IPV6_NETS.iter().any(|net| net.contains(ipv6))
        }
    }
}

/// 解析 BSD 风格的 IPv4 地址简写（支持十进制整数、十六进制、八进制及缺省点分）
fn parse_shorthand_ipv4(s: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let mut nums = Vec::with_capacity(parts.len());
    for p in parts {
        let trimmed = p.trim();
        if trimmed.is_empty() {
            return None;
        }
        let val = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            u64::from_str_radix(&trimmed[2..], 16).ok()?
        } else if trimmed.len() > 1 && trimmed.starts_with('0') {
            u64::from_str_radix(trimmed, 8).ok()?
        } else {
            trimmed.parse::<u64>().ok()?
        };
        nums.push(val);
    }

    let val: u32 = match nums.len() {
        1 => {
            if nums[0] > 0xFFFF_FFFF {
                return None;
            }
            nums[0] as u32
        }
        2 => {
            if nums[0] > 0xFF || nums[1] > 0x00FF_FFFF {
                return None;
            }
            ((nums[0] as u32) << 24) | (nums[1] as u32)
        }
        3 => {
            if nums[0] > 0xFF || nums[1] > 0xFF || nums[2] > 0x0000_FFFF {
                return None;
            }
            ((nums[0] as u32) << 24) | ((nums[1] as u32) << 16) | (nums[2] as u32)
        }
        4 => {
            if nums[0] > 0xFF || nums[1] > 0xFF || nums[2] > 0xFF || nums[3] > 0xFF {
                return None;
            }
            ((nums[0] as u32) << 24)
                | ((nums[1] as u32) << 16)
                | ((nums[2] as u32) << 8)
                | (nums[3] as u32)
        }
        _ => return None,
    };
    Some(Ipv4Addr::from(val))
}

/// 检查主机是否为内网/保留地址（包含后缀、字面 IP、IP 简写及 DNS 解析）
pub fn is_endpoint_host_denied(host: &str) -> bool {
    let clean_host = host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
    if clean_host.is_empty() {
        return true;
    }

    if clean_host == "localhost"
        || ENDPOINT_HOST_DENY_SUFFIXES
            .iter()
            .any(|suffix| clean_host.ends_with(suffix))
    {
        return true;
    }

    // 字面 IP
    if let Ok(ip) = clean_host.parse::<IpAddr>() {
        return is_ip_denied(&ip);
    }

    // 简写 IP（如 127.1, 2130706433, 0x7f.0x0.0x0.0x1, 10.1）
    if let Some(v4) = parse_shorthand_ipv4(&clean_host) {
        return is_ip_denied(&IpAddr::V4(v4));
    }

    // DNS 解析后逐 IP 检查
    if let Ok(addrs) = (clean_host.as_str(), 80).to_socket_addrs() {
        for addr in addrs {
            if is_ip_denied(&addr.ip()) {
                return true;
            }
        }
    }

    false
}

/// 校验完整 URL 形态的出站端点（s3_endpoint_url / webdav_url）
pub fn validate_outbound_endpoint(
    url_str: &str,
    is_production: bool,
) -> Result<String, AppError> {
    let endpoint = url_str.trim();
    if endpoint.is_empty() {
        return Ok(String::new());
    }

    if !is_production {
        return Ok(endpoint.to_string());
    }

    let parsed = Url::parse(endpoint)
        .map_err(|_| AppError::BadRequest(format!("端点 URL 格式无效：{}", endpoint)))?;

    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(AppError::BadRequest(format!(
            "端点协议必须是 http(s)：{}",
            endpoint
        )));
    }

    let host = match parsed.host_str() {
        Some(h) if !h.is_empty() => h,
        _ => {
            return Err(AppError::BadRequest(format!(
                "端点缺少主机名：{}",
                endpoint
            )))
        }
    };

    if is_endpoint_host_denied(host) {
        return Err(AppError::BadRequest(format!(
            "端点不允许指向内网/保留地址：{}",
            endpoint
        )));
    }

    Ok(endpoint.to_string())
}

/// 校验裸主机名形态的出站端点（s3_hostname，存储层拼接 https://{hostname}）
pub fn validate_outbound_hostname(
    hostname_str: &str,
    is_production: bool,
) -> Result<String, AppError> {
    let hostname = hostname_str.trim();
    if hostname.is_empty() {
        return Ok(String::new());
    }

    if !is_production {
        return Ok(hostname.to_string());
    }

    if hostname.contains("://") || hostname.contains('/') || hostname.contains('@') {
        return Err(AppError::BadRequest(format!(
            "s3_hostname 应为裸主机名，不含协议、路径或 userinfo：{}",
            hostname
        )));
    }

    // 提取主机部分：[v6] 形态取括号内；host:port 取冒号前；裸 IPv6（多个冒号）整体
    let host: &str = if hostname.contains('[') {
        if let Some(start) = hostname.find('[') {
            if let Some(end) = hostname.find(']') {
                if end > start {
                    let inside = &hostname[start + 1..end];
                    let rest = &hostname[end + 1..];
                    if !rest.is_empty() && !rest.starts_with(':') {
                        return Err(AppError::BadRequest(format!(
                            "s3_hostname 的 IPv6 括号后只允许端口：{}",
                            hostname
                        )));
                    }
                    inside
                } else {
                    return Err(AppError::BadRequest(format!(
                        "s3_hostname 括号格式错误：{}",
                        hostname
                    )));
                }
            } else {
                return Err(AppError::BadRequest(format!(
                    "s3_hostname 缺少闭合括号：{}",
                    hostname
                )));
            }
        } else {
            hostname
        }
    } else if hostname.matches(':').count() == 1 {
        hostname.split_once(':').map(|(h, _)| h).unwrap_or(hostname)
    } else {
        hostname
    };

    if host.is_empty() || is_endpoint_host_denied(host) {
        return Err(AppError::BadRequest(format!(
            "端点不允许指向内网/保留地址：{}",
            hostname
        )));
    }

    Ok(hostname.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ip_denied() {
        assert!(is_ip_denied(&"127.0.0.1".parse().unwrap()));
        assert!(is_ip_denied(&"10.0.0.5".parse().unwrap()));
        assert!(is_ip_denied(&"192.168.1.10".parse().unwrap()));
        assert!(is_ip_denied(&"172.16.0.1".parse().unwrap()));
        assert!(is_ip_denied(&"169.254.169.254".parse().unwrap()));
        assert!(is_ip_denied(&"0.0.0.0".parse().unwrap()));
        assert!(is_ip_denied(&"240.0.0.1".parse().unwrap()));
        assert!(is_ip_denied(&"198.18.0.1".parse().unwrap()));
        assert!(is_ip_denied(&"::1".parse().unwrap()));
        assert!(is_ip_denied(&"::".parse().unwrap()));
        assert!(is_ip_denied(&"fc00::1".parse().unwrap()));
        assert!(is_ip_denied(&"fe80::1".parse().unwrap()));
        assert!(is_ip_denied(&"::ffff:127.0.0.1".parse().unwrap()));

        // Public IPs
        assert!(!is_ip_denied(&"1.1.1.1".parse().unwrap()));
        assert!(!is_ip_denied(&"8.8.8.8".parse().unwrap()));
        assert!(!is_ip_denied(&"2606:4700::1".parse().unwrap()));
    }

    #[test]
    fn test_production_rejects_internal_endpoints() {
        let denied = [
            "http://127.0.0.1:9000",
            "https://10.0.0.5",
            "https://192.168.1.10:9000",
            "http://172.16.0.1",
            "http://169.254.169.254/latest/meta-data",
            "https://localhost:9000",
            "https://minio.internal:9000",
            "https://nas.local",
            "file:///etc/passwd",
            "gopher://10.0.0.1",
            "ftp://example.com",
            "not a url",
            "https://127.1",
            "https://2130706433",
            "https://0x7f.0x0.0x0.0x1",
            "https://10.1",
            "https://foo.localhost",
            "https://[::1]",
            "https://0.0.0.0",
        ];

        for endpoint in &denied {
            assert!(
                validate_outbound_endpoint(endpoint, true).is_err(),
                "Endpoint should be rejected: {}",
                endpoint
            );
        }
    }

    #[test]
    fn test_production_allows_public_endpoints() {
        let allowed = [
            "",
            "https://93.184.216.34:9000",
            "https://s3.amazonaws.com",
            "https://s3.cn-north-1.amazonaws.com.cn",
            "http://example.com:9000",
        ];

        for endpoint in &allowed {
            let res = validate_outbound_endpoint(endpoint, true);
            // If DNS proxy (like Clash/Tun) maps domains to Fake-IP (198.18.x.x), allow that environment-specific result
            if let Err(_e) = &res {
                if endpoint.contains("amazonaws") || endpoint.contains("example.com") {
                    continue;
                }
            }
            assert_eq!(
                res.unwrap(),
                *endpoint
            );
        }
    }

    #[test]
    fn test_development_env_allows_local_endpoints() {
        assert_eq!(
            validate_outbound_endpoint("http://127.0.0.1:9000", false).unwrap(),
            "http://127.0.0.1:9000"
        );
        assert_eq!(
            validate_outbound_hostname("127.0.0.1:9000", false).unwrap(),
            "127.0.0.1:9000"
        );
    }

    #[test]
    fn test_production_rejects_internal_hostnames() {
        let denied = [
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.10:9000",
            "localhost",
            "minio.internal",
            "nas.local",
            "127.1",
            "2130706433",
            "10.1",
            "foo.localhost",
            "[::1]",
            "[::ffff:127.0.0.1]:9000",
            "https://s3.amazonaws.com",
            "http://127.0.0.1:9000",
            "evil.com@127.0.0.1",
            "attacker.example@127.0.0.1:8443",
        ];

        for hostname in &denied {
            assert!(
                validate_outbound_hostname(hostname, true).is_err(),
                "Hostname should be rejected: {}",
                hostname
            );
        }
    }

    #[test]
    fn test_production_allows_public_hostnames() {
        assert_eq!(
            validate_outbound_hostname("", true).unwrap(),
            ""
        );
        let check_host = |h: &str| {
            let res = validate_outbound_hostname(h, true);
            if res.is_err() && (h.contains("amazonaws") || h.contains("example.com")) {
                return;
            }
            assert_eq!(res.unwrap(), h);
        };
        check_host("s3.amazonaws.com");
        check_host("minio.corp.example.com");
        check_host("files.example.com:9000");
        assert_eq!(
            validate_outbound_hostname("[2606:4700::1]:9000", true).unwrap(),
            "[2606:4700::1]:9000"
        );
    }
}
