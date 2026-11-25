//! Windows OS integration layer (Start Menu app scan, Win32 foreground overlap,
//! power/lock/screenshot/clipboard actions) with compile-safe cross-platform fallbacks.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WinApp {
    pub name: String,
    pub target: String,
    pub comment: String,
    pub is_action: bool,
}

pub fn data_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("atlantis")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("atlantis")
    } else {
        PathBuf::from(".")
    }
}

pub fn scan_apps() -> Vec<WinApp> {
    let mut apps = Vec::new();
    #[cfg(windows)]
    {
        let mut dirs = Vec::new();
        if let Ok(pd) = std::env::var("ProgramData") {
            dirs.push(PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
        if let Ok(ad) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(ad).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
        for dir in dirs {
            collect_lnk_recursive(&dir, &mut apps);
        }
    }
    if apps.is_empty() {
        for (name, target, comment) in [
            ("Terminal", "wt.exe", "Windows Terminal"),
            ("Explorer", "explorer.exe", "File Explorer"),
            ("Browser", "msedge.exe", "Microsoft Edge"),
            ("Settings", "ms-settings:", "Windows Settings"),
            ("Task Manager", "taskmgr.exe", "System Performance"),
            ("Notepad", "notepad.exe", "Text Editor"),
        ] {
            apps.push(WinApp {
                name: name.into(),
                target: target.into(),
                comment: comment.into(),
                is_action: false,
            });
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    apps
}

#[cfg(windows)]
fn collect_lnk_recursive(dir: &std::path::Path, out: &mut Vec<WinApp>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_lnk_recursive(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("lnk") {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if !stem.to_lowercase().contains("uninstall") {
                    out.push(WinApp {
                        name: stem.to_string(),
                        target: p.to_string_lossy().to_string(),
                        comment: "Start Menu Application".into(),
                        is_action: false,
                    });
                }
            }
        }
    }
}

pub fn launch_app(target: &str) {
    #[cfg(windows)]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", target]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("sh").args(["-c", target]).spawn();
    }
}

pub fn copy_to_clipboard(text: &str) {
    #[cfg(windows)]
    {
        use std::io::Write;
        if let Ok(mut child) = Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("wl-copy").arg(text).spawn();
    }
}

pub fn lock_screen() {
    #[cfg(windows)]
    {
        let _ = Command::new("rundll32.exe")
            .arg("user32.dll,LockWorkStation")
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("loginctl").arg("lock-session").spawn();
    }
}

pub fn suspend() {
    #[cfg(windows)]
    {
        let _ = Command::new("rundll32.exe")
            .arg("powrprof.dll,SetSuspendState 0,1,0")
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("systemctl").arg("suspend").spawn();
    }
}

pub fn reboot() {
    #[cfg(windows)]
    {
        let _ = Command::new("shutdown").args(["/r", "/t", "0"]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("systemctl").arg("reboot").spawn();
    }
}

pub fn poweroff() {
    #[cfg(windows)]
    {
        let _ = Command::new("shutdown").args(["/s", "/t", "0"]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("systemctl").arg("poweroff").spawn();
    }
}

pub fn take_screenshot() {
    #[cfg(windows)]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", "ms-screenclip:"])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("grim").spawn();
    }
}

pub fn adjust_volume(delta: i32) {
    #[cfg(windows)]
    {
        let script = if delta == 0 {
            "(New-Object -ComObject WScript.Shell).SendKeys([char]173)"
        } else if delta > 0 {
            "(New-Object -ComObject WScript.Shell).SendKeys([char]175)"
        } else {
            "(New-Object -ComObject WScript.Shell).SendKeys([char]174)"
        };
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", script])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = delta;
    }
}

pub fn adjust_brightness(pct: u32) {
    #[cfg(windows)]
    {
        let script = format!(
            "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1,{})",
            pct.clamp(5, 100)
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = pct;
    }
}

pub fn query_battery() -> (u32, bool) {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemPowerStatus {
            ac_line_status: u8,
            battery_flag: u8,
            battery_life_percent: u8,
            system_status_flag: u8,
            battery_life_time: u32,
            battery_full_life_time: u32,
        }
        extern "system" {
            fn GetSystemPowerStatus(lp_system_power_status: *mut SystemPowerStatus) -> i32;
        }
        let mut s = SystemPowerStatus {
            ac_line_status: 1,
            battery_flag: 0,
            battery_life_percent: 100,
            system_status_flag: 0,
            battery_life_time: 0,
            battery_full_life_time: 0,
        };
        if unsafe { GetSystemPowerStatus(&mut s) } != 0 {
            let pct = if s.battery_life_percent <= 100 {
                s.battery_life_percent as u32
            } else {
                100
            };
            return (pct, s.ac_line_status == 1);
        }
    }
    (100, true)
}

pub fn is_foreground_overlapping(rx: i32, ry: i32, rw: i32, rh: i32) -> bool {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Rect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        extern "system" {
            fn GetForegroundWindow() -> isize;
            fn GetWindowRect(hwnd: isize, lp_rect: *mut Rect) -> i32;
            fn IsZoomed(hwnd: isize) -> i32;
        }
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 {
                return false;
            }
            if IsZoomed(hwnd) != 0 {
                return true;
            }
            let mut r = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut r) != 0 {
                let rr = rx + rw;
                let rb = ry + rh;
                return r.left < rr && r.right > rx && r.top < rb && r.bottom > ry;
            }
        }
    }
    let _ = (rx, ry, rw, rh);
    false
}
