//! 后台任务（安装、下载 Java 等）的进度跟踪，并通过 `task-update` 事件推送到前端。

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::util::now_ms;

pub const TASK_EVENT: &str = "task-update";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub stage: String,
    pub stage_index: u32,
    pub stage_count: u32,
    /// running / success / failed / cancelled
    pub status: String,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// 字节/秒
    pub speed: u64,
    /// 当前阶段进度 0~1
    pub progress: f64,
    pub error: Option<String>,
    pub created_at: u64,
    pub finished_at: Option<u64>,
}

struct Mutable {
    stage: String,
    stage_index: u32,
    stage_count: u32,
    status: String,
    error: Option<String>,
    finished_at: Option<u64>,
    last_emit: Option<Instant>,
    speed_mark: (Instant, u64),
    speed: u64,
}

pub struct TaskHandle {
    pub id: String,
    pub title: String,
    pub kind: String,
    app: AppHandle,
    created_at: u64,
    files_done: AtomicU64,
    files_total: AtomicU64,
    bytes_done: AtomicU64,
    bytes_total: AtomicU64,
    cancelled: AtomicBool,
    m: Mutex<Mutable>,
}

impl TaskHandle {
    fn lock(&self) -> std::sync::MutexGuard<'_, Mutable> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 进入新阶段，重置计数器
    pub fn set_stage(&self, index: u32, count: u32, stage: impl Into<String>, files_total: u64, bytes_total: u64) {
        self.files_done.store(0, Ordering::Relaxed);
        self.bytes_done.store(0, Ordering::Relaxed);
        self.files_total.store(files_total, Ordering::Relaxed);
        self.bytes_total.store(bytes_total, Ordering::Relaxed);
        {
            let mut m = self.lock();
            m.stage = stage.into();
            m.stage_index = index;
            m.stage_count = count;
            m.speed_mark = (Instant::now(), 0);
            m.speed = 0;
        }
        self.emit(true);
    }

    pub fn add_progress(&self, files: u64, bytes: u64) {
        if files > 0 {
            self.files_done.fetch_add(files, Ordering::Relaxed);
        }
        if bytes > 0 {
            self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        }
        self.emit(false);
    }

    pub fn add_bytes(&self, bytes: u64) {
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        self.emit(false);
    }

    /// 下载失败重试时，撤回已经计入的字节
    pub fn sub_bytes(&self, bytes: u64) {
        let mut cur = self.bytes_done.load(Ordering::Relaxed);
        loop {
            let next = cur.saturating_sub(bytes);
            match self.bytes_done.compare_exchange_weak(cur, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.emit(true);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.lock().status != "running"
    }

    pub fn finish(&self, result: &anyhow::Result<()>) {
        {
            let mut m = self.lock();
            match result {
                Ok(()) => {
                    m.status = "success".into();
                    m.stage = "已完成".into();
                }
                Err(e) => {
                    if self.is_cancelled() {
                        m.status = "cancelled".into();
                        m.stage = "已取消".into();
                    } else {
                        m.status = "failed".into();
                        m.error = Some(format!("{:#}", e));
                    }
                }
            }
            m.finished_at = Some(now_ms());
        }
        self.emit(true);
    }

    pub fn snapshot(&self) -> TaskSnapshot {
        let files_done = self.files_done.load(Ordering::Relaxed);
        let files_total = self.files_total.load(Ordering::Relaxed);
        let bytes_done = self.bytes_done.load(Ordering::Relaxed);
        let bytes_total = self.bytes_total.load(Ordering::Relaxed);
        let m = self.lock();
        let progress = if m.status == "success" {
            1.0
        } else if bytes_total > 0 {
            (bytes_done as f64 / bytes_total as f64).min(1.0)
        } else if files_total > 0 {
            (files_done as f64 / files_total as f64).min(1.0)
        } else {
            0.0
        };
        TaskSnapshot {
            id: self.id.clone(),
            title: self.title.clone(),
            kind: self.kind.clone(),
            stage: m.stage.clone(),
            stage_index: m.stage_index,
            stage_count: m.stage_count,
            status: m.status.clone(),
            files_done,
            files_total,
            bytes_done,
            bytes_total,
            speed: m.speed,
            progress,
            error: m.error.clone(),
            created_at: self.created_at,
            finished_at: m.finished_at,
        }
    }

    /// 节流推送：非强制时 150ms 内最多推送一次
    fn emit(&self, force: bool) {
        let now = Instant::now();
        {
            let mut m = self.lock();
            if !force {
                if let Some(t) = m.last_emit {
                    if now.duration_since(t) < Duration::from_millis(150) {
                        return;
                    }
                }
            }
            m.last_emit = Some(now);
            let bytes = self.bytes_done.load(Ordering::Relaxed);
            let (t0, b0) = m.speed_mark;
            let dt = now.duration_since(t0).as_secs_f64();
            if dt >= 0.5 {
                m.speed = (bytes.saturating_sub(b0) as f64 / dt) as u64;
                m.speed_mark = (now, bytes);
            }
        }
        let snap = self.snapshot();
        let _ = self.app.emit(TASK_EVENT, snap);
    }
}

pub struct TaskManager {
    app: AppHandle,
    tasks: Mutex<Vec<Arc<TaskHandle>>>,
}

impl TaskManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app, tasks: Mutex::new(Vec::new()) }
    }

    pub fn create(&self, title: impl Into<String>, kind: impl Into<String>) -> Arc<TaskHandle> {
        let now = Instant::now();
        let task = Arc::new(TaskHandle {
            id: uuid::Uuid::new_v4().simple().to_string(),
            title: title.into(),
            kind: kind.into(),
            app: self.app.clone(),
            created_at: now_ms(),
            files_done: AtomicU64::new(0),
            files_total: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            m: Mutex::new(Mutable {
                stage: "准备中".into(),
                stage_index: 0,
                stage_count: 0,
                status: "running".into(),
                error: None,
                finished_at: None,
                last_emit: None,
                speed_mark: (now, 0),
                speed: 0,
            }),
        });
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).push(task.clone());
        task.emit(true);
        task
    }

    pub fn list(&self) -> Vec<TaskSnapshot> {
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|t| t.snapshot())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<Arc<TaskHandle>> {
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    pub fn clear_finished(&self) {
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|t| !t.is_finished());
    }
}

/// 在后台运行任务，结束时自动写入成功/失败状态
pub fn spawn_task<F, Fut>(task: Arc<TaskHandle>, f: F)
where
    F: FnOnce(Arc<TaskHandle>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let result = f(task.clone()).await;
        if let Err(e) = &result {
            eprintln!("[task {}] 失败：{:#}", task.title, e);
        }
        task.finish(&result);
    });
}
