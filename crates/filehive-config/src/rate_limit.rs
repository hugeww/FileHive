use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use arc_swap::ArcSwap;
use filehive_core::AppError;

use crate::settings::SystemSettings;

/// 内存 IP 限流器，基于滑动窗口算法
#[derive(Clone)]
pub struct RateLimiter {
    settings: Arc<ArcSwap<SystemSettings>>,
    records: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    /// 基于系统设置初始化限流器
    pub fn new(settings: Arc<ArcSwap<SystemSettings>>) -> Self {
        Self {
            settings,
            records: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 获取指定分类的限制阈值 (count, minutes)
    pub fn get_thresholds(&self, category: &str) -> Result<(i32, i32), AppError> {
        let settings = self.settings.load();
        match category {
            "upload" => Ok((settings.upload_count, settings.upload_minute)),
            "login" => Ok((settings.login_count, settings.login_minute)),
            "error" => Ok((settings.error_count, settings.error_minute)),
            "metadata" => Ok((settings.error_count, settings.error_minute)),
            other => Err(AppError::BadRequest(format!(
                "Unsupported rate limit category: {}",
                other
            ))),
        }
    }

    /// 校验 IP 并在未超出限制时记录一次访问
    /// 如果超出限制则返回 HTTP 429 (TooManyRequests)
    pub fn check_ip(&self, ip: &str, category: &str) -> Result<(), AppError> {
        let (limit_count, limit_minutes) = self.get_thresholds(category)?;
        if limit_count <= 0 || limit_minutes <= 0 {
            return Ok(());
        }

        let key = format!("{}:{}", category, ip);
        let now = Instant::now();
        let window = Duration::from_secs((limit_minutes as u64) * 60);

        let mut map = self.records.lock().unwrap();
        let entries = map.entry(key).or_default();

        // 滑动窗口：剔除窗口外的历史记录
        entries.retain(|&time| now.duration_since(time) < window);

        if entries.len() >= limit_count as usize {
            return Err(AppError::TooManyRequests(
                "请求过于频繁，请稍后再试".to_string(),
            ));
        }

        entries.push(now);
        Ok(())
    }

    /// 仅检查 IP 是否超出限制，不记录访问次数
    pub fn check_only(&self, ip: &str, category: &str) -> Result<(), AppError> {
        let (limit_count, limit_minutes) = self.get_thresholds(category)?;
        if limit_count <= 0 || limit_minutes <= 0 {
            return Ok(());
        }

        let key = format!("{}:{}", category, ip);
        let now = Instant::now();
        let window = Duration::from_secs((limit_minutes as u64) * 60);

        let mut map = self.records.lock().unwrap();
        if let Some(entries) = map.get_mut(&key) {
            entries.retain(|&time| now.duration_since(time) < window);
            if entries.len() >= limit_count as usize {
                return Err(AppError::TooManyRequests(
                    "请求过于频繁，请稍后再试".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// 手动记录一次访问（常用于密码错误、取件码错误等失败计数）
    pub fn add_ip(&self, ip: &str, category: &str) {
        let key = format!("{}:{}", category, ip);
        let now = Instant::now();
        let mut map = self.records.lock().unwrap();
        let entries = map.entry(key).or_default();
        entries.push(now);
    }

    /// 清空所有限流记录
    pub fn clear(&self) {
        let mut map = self.records.lock().unwrap();
        map.clear();
    }

    /// 清理所有分类已过期的 IP 记录，释放内存
    pub fn clean_expired(&self) {
        let mut map = self.records.lock().unwrap();
        let now = Instant::now();
        map.retain(|key, entries| {
            let category = key.split(':').next().unwrap_or("");
            let limit_minutes = match category {
                "upload" => self.settings.load().upload_minute,
                "login" => self.settings.load().login_minute,
                "error" | "metadata" => self.settings.load().error_minute,
                _ => 1,
            };
            let window = Duration::from_secs((limit_minutes.max(1) as u64) * 60);
            entries.retain(|&time| now.duration_since(time) < window);
            !entries.is_empty()
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(Arc::new(ArcSwap::from_pointee(SystemSettings::default())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_supported_categories() {
        let limiter = RateLimiter::default();
        assert!(limiter.get_thresholds("upload").is_ok());
        assert!(limiter.get_thresholds("login").is_ok());
        assert!(limiter.get_thresholds("error").is_ok());
        assert!(limiter.get_thresholds("metadata").is_ok());
        assert!(limiter.get_thresholds("unknown").is_err());
    }

    #[test]
    fn test_rate_limiter_exceeded_returns_429() {
        let mut settings = SystemSettings::default();
        settings.login_count = 3;
        settings.login_minute = 10;
        let limiter = RateLimiter::new(Arc::new(ArcSwap::from_pointee(settings)));

        let ip = "192.168.1.100";
        assert!(limiter.check_ip(ip, "login").is_ok());
        assert!(limiter.check_ip(ip, "login").is_ok());
        assert!(limiter.check_ip(ip, "login").is_ok());

        // 第4次应触发限流
        let err = limiter.check_ip(ip, "login").unwrap_err();
        assert_eq!(err.status_code(), 429);

        // 另一个 IP 不受影响
        assert!(limiter.check_ip("192.168.1.101", "login").is_ok());
    }

    #[test]
    fn test_rate_limiter_check_only_and_add_ip() {
        let mut settings = SystemSettings::default();
        settings.error_count = 2;
        settings.error_minute = 5;
        let limiter = RateLimiter::new(Arc::new(ArcSwap::from_pointee(settings)));

        let ip = "10.0.0.1";
        // check_only 不消耗配额
        assert!(limiter.check_only(ip, "error").is_ok());
        assert!(limiter.check_only(ip, "error").is_ok());
        assert!(limiter.check_only(ip, "error").is_ok());

        // add_ip 消耗配额
        limiter.add_ip(ip, "error");
        limiter.add_ip(ip, "error");

        // 达到上限
        let err = limiter.check_only(ip, "error").unwrap_err();
        assert_eq!(err.status_code(), 429);

        // 清除后恢复
        limiter.clear();
        assert!(limiter.check_only(ip, "error").is_ok());
    }

    #[test]
    fn test_rate_limiter_dynamic_settings() {
        let settings = Arc::new(ArcSwap::from_pointee(SystemSettings::default()));
        let limiter = RateLimiter::new(settings.clone());

        assert_eq!(limiter.get_thresholds("upload").unwrap(), (10, 1));

        // 模拟设置热更新
        let mut updated = (**settings.load()).clone();
        updated.upload_count = 50;
        updated.upload_minute = 5;
        settings.store(Arc::new(updated));

        assert_eq!(limiter.get_thresholds("upload").unwrap(), (50, 5));
    }
}
