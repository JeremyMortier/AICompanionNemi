use anyhow::Result;
use sysinfo::System;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId,
};

#[derive(Debug, Clone)]
pub struct ActiveWindowInfo {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
    pub rect: WindowRect,
}

pub fn get_active_window_info() -> Result<Option<ActiveWindowInfo>> {
    let hwnd = unsafe { GetForegroundWindow() };
    let rect = get_window_rect(hwnd)?;

    if hwnd.0.is_null() {
        return Ok(None);
    }

    let title = get_window_title(hwnd)?;
    if title.trim().is_empty() {
        return Ok(None);
    }

    let process_id = get_window_process_id(hwnd);
    let process_name = get_process_name(process_id).unwrap_or_else(|| "unknown".to_string());

    Ok(Some(ActiveWindowInfo {
        title,
        process_id,
        process_name,
        rect,
    }))
}

fn get_window_title(hwnd: HWND) -> Result<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };

    if length == 0 {
        return Ok(String::new());
    }

    let mut buffer = vec![0u16; (length + 1) as usize];

    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };

    let title = String::from_utf16_lossy(&buffer[..copied as usize]);
    Ok(title)
}

fn get_window_process_id(hwnd: HWND) -> u32 {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    process_id
}

fn get_process_name(pid: u32) -> Option<String> {
    let system = System::new_all();

    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(|process| process.name().to_string())
}

#[derive(Debug, Clone)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

// impl WindowRect {
//     pub fn center_x(&self) -> i32 {
//         (self.left + self.right) / 2
//     }

//     pub fn center_y(&self) -> i32 {
//         (self.top + self.bottom) / 2
//     }
// }

fn get_window_rect(hwnd: HWND) -> Result<WindowRect> {
    let mut rect = RECT::default();

    unsafe {
        GetWindowRect(hwnd, &mut rect)?;
    }

    Ok(WindowRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}
