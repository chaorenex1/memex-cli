//! STDIO 性能监控模块（Level 5）
//!
//! 提供原子化的性能指标收集和报告功能，用于优化分析和基准测试。

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// STDIO 性能指标
pub struct StdioMetrics {
    /// 任务解析耗时（纳秒）
    pub parse_time_ns: AtomicU64,

    /// 文件解析耗时（纳秒）
    pub file_resolve_time_ns: AtomicU64,

    /// 文件读取字节数
    pub file_read_bytes: AtomicU64,

    /// 事件输出数量
    pub events_emitted: AtomicU64,

    /// 缓存命中次数
    pub cache_hits: AtomicU64,

    /// 缓存未命中次数
    pub cache_misses: AtomicU64,

    /// 并发调整次数
    pub concurrency_adjustments: AtomicU64,

    /// mmap 使用次数
    pub mmap_operations: AtomicU64,

    /// SIMD 检测次数
    pub simd_detections: AtomicU64,
}

impl StdioMetrics {
    /// 创建新的性能指标实例
    pub fn new() -> Self {
        Self {
            parse_time_ns: AtomicU64::new(0),
            file_resolve_time_ns: AtomicU64::new(0),
            file_read_bytes: AtomicU64::new(0),
            events_emitted: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            concurrency_adjustments: AtomicU64::new(0),
            mmap_operations: AtomicU64::new(0),
            simd_detections: AtomicU64::new(0),
        }
    }

    /// 记录解析耗时
    pub fn record_parse_time(&self, duration_ns: u64) {
        self.parse_time_ns.fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// 记录文件解析耗时
    pub fn record_file_resolve_time(&self, duration_ns: u64) {
        self.file_resolve_time_ns
            .fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// 记录文件读取字节数
    pub fn record_file_read_bytes(&self, bytes: u64) {
        self.file_read_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 记录事件输出
    pub fn record_event_emitted(&self) {
        self.events_emitted.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录并发调整
    pub fn record_concurrency_adjustment(&self) {
        self.concurrency_adjustments.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 mmap 操作
    pub fn record_mmap_operation(&self) {
        self.mmap_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 SIMD 检测
    pub fn record_simd_detection(&self) {
        self.simd_detections.fetch_add(1, Ordering::Relaxed);
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.parse_time_ns.store(0, Ordering::Relaxed);
        self.file_resolve_time_ns.store(0, Ordering::Relaxed);
        self.file_read_bytes.store(0, Ordering::Relaxed);
        self.events_emitted.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.concurrency_adjustments.store(0, Ordering::Relaxed);
        self.mmap_operations.store(0, Ordering::Relaxed);
        self.simd_detections.store(0, Ordering::Relaxed);
    }

    /// 生成性能报告
    pub fn report(&self) {
        let parse_ms = self.parse_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let file_resolve_ms =
            self.file_resolve_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let file_read_mb = self.file_read_bytes.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let events = self.events_emitted.load(Ordering::Relaxed);
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let concurrency_adj = self.concurrency_adjustments.load(Ordering::Relaxed);
        let mmap_ops = self.mmap_operations.load(Ordering::Relaxed);
        let simd_ops = self.simd_detections.load(Ordering::Relaxed);

        eprintln!();
        eprintln!("╔════════════════════════════════════════════════════════════╗");
        eprintln!("║           STDIO 协议性能监控报告 (Level 5)                 ║");
        eprintln!("╠════════════════════════════════════════════════════════════╣");
        eprintln!("║ 解析性能                                                    ║");
        eprintln!(
            "║   任务解析耗时: {:.2} ms                                 ",
            parse_ms
        );
        eprintln!(
            "║   文件解析耗时: {:.2} ms                                 ",
            file_resolve_ms
        );
        eprintln!("╠════════════════════════════════════════════════════════════╣");
        eprintln!("║ 文件 I/O                                                    ║");
        eprintln!(
            "║   读取字节数: {:.2} MB                                   ",
            file_read_mb
        );
        eprintln!(
            "║   mmap 操作次数: {}                                      ",
            mmap_ops
        );
        eprintln!("╠════════════════════════════════════════════════════════════╣");
        eprintln!("║ 事件输出                                                    ║");
        eprintln!(
            "║   输出事件数: {}                                         ",
            events
        );
        eprintln!("╠════════════════════════════════════════════════════════════╣");
        eprintln!("║ 缓存效率 (Level 3.3)                                        ║");

        if hits + misses > 0 {
            let hit_rate = hits as f64 / (hits + misses) as f64 * 100.0;
            eprintln!(
                "║   缓存命中率: {:.1}% ({}/{})                         ",
                hit_rate,
                hits,
                hits + misses
            );
            eprintln!(
                "║   缓存命中: {}                                          ",
                hits
            );
            eprintln!(
                "║   缓存未命中: {}                                        ",
                misses
            );
        } else {
            eprintln!("║   缓存未启用或无访问                                    ║");
        }

        eprintln!("╠════════════════════════════════════════════════════════════╣");
        eprintln!("║ 优化特性使用统计                                            ║");
        eprintln!(
            "║   动态并发调整次数 (Level 2.2): {}                      ",
            concurrency_adj
        );
        eprintln!(
            "║   SIMD 检测次数 (Level 3.2): {}                         ",
            simd_ops
        );
        eprintln!("╚════════════════════════════════════════════════════════════╝");
        eprintln!();
    }
}

impl Default for StdioMetrics {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static! {
    /// 全局 STDIO 性能指标实例
    pub static ref STDIO_METRICS: StdioMetrics = StdioMetrics::new();
}

/// 性能计时器辅助结构
pub struct PerfTimer {
    start: Instant,
    metric: MetricType,
}

/// 指标类型
pub enum MetricType {
    Parse,
    FileResolve,
}

impl PerfTimer {
    /// 开始计时
    pub fn start(metric: MetricType) -> Self {
        Self {
            start: Instant::now(),
            metric,
        }
    }
}

impl Drop for PerfTimer {
    fn drop(&mut self) {
        let elapsed_ns = self.start.elapsed().as_nanos() as u64;
        match self.metric {
            MetricType::Parse => STDIO_METRICS.record_parse_time(elapsed_ns),
            MetricType::FileResolve => STDIO_METRICS.record_file_resolve_time(elapsed_ns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_record_and_report() {
        let metrics = StdioMetrics::new();

        metrics.record_parse_time(1_000_000); // 1ms
        metrics.record_file_resolve_time(5_000_000); // 5ms
        metrics.record_file_read_bytes(1024 * 1024); // 1MB
        metrics.record_event_emitted();
        metrics.record_event_emitted();
        metrics.record_cache_hit();
        metrics.record_cache_miss();

        assert_eq!(metrics.parse_time_ns.load(Ordering::Relaxed), 1_000_000);
        assert_eq!(
            metrics.file_resolve_time_ns.load(Ordering::Relaxed),
            5_000_000
        );
        assert_eq!(metrics.file_read_bytes.load(Ordering::Relaxed), 1024 * 1024);
        assert_eq!(metrics.events_emitted.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.cache_hits.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.cache_misses.load(Ordering::Relaxed), 1);

        metrics.report(); // 应该打印报告
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = StdioMetrics::new();

        metrics.record_parse_time(1_000_000);
        metrics.record_event_emitted();
        metrics.reset();

        assert_eq!(metrics.parse_time_ns.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.events_emitted.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_perf_timer() {
        // 重置全局指标
        STDIO_METRICS.reset();

        {
            let _timer = PerfTimer::start(MetricType::Parse);
            // 使用更长的 sleep 时间确保在所有平台都能记录到
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // 使用全局 STDIO_METRICS（因为 PerfTimer 记录到全局实例）
        let elapsed = STDIO_METRICS.parse_time_ns.load(Ordering::Relaxed);
        assert!(elapsed > 0, "Timer should record non-zero time");
        // 验证时间至少大于 5ms（考虑调度延迟）
        assert!(
            elapsed > 5_000_000,
            "Timer should record at least 5ms, got {} ns",
            elapsed
        );
    }
}
