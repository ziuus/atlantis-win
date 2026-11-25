//! Cross-platform hardware & session telemetry for Atlantis Windows.

use std::time::{Duration, Instant};
use sysinfo::{Disks, System};

#[derive(Debug)]
pub struct SysStats {
    pub cpu_pct: u32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub bat_pct: u32,
    pub bat_charging: bool,
    pub uptime_hrs: u32,
    pub uptime_mins: u32,
    pub vol_pct: u32,
    pub vol_muted: bool,
    pub brightness_pct: u32,
    #[allow(dead_code)]
    pub wifi_enabled: bool,
    #[allow(dead_code)]
    pub bt_enabled: bool,
    pub disk_pct: u32,
    sys: System,
    disks: Disks,
    last_poll: Instant,
}

impl SysStats {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let mut s = Self {
            cpu_pct: 0,
            ram_used_gb: 0.0,
            ram_total_gb: 1.0,
            bat_pct: 100,
            bat_charging: true,
            uptime_hrs: 0,
            uptime_mins: 0,
            vol_pct: 50,
            vol_muted: false,
            brightness_pct: 75,
            wifi_enabled: true,
            bt_enabled: true,
            disk_pct: 0,
            sys,
            disks,
            last_poll: Instant::now() - Duration::from_secs(10),
        };
        s.poll(false);
        s
    }

    pub fn poll(&mut self, power_save: bool) {
        let interval = if power_save {
            Duration::from_secs(8)
        } else {
            Duration::from_millis(1800)
        };
        if self.last_poll.elapsed() < interval {
            return;
        }
        self.last_poll = Instant::now();
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.cpu_pct = self.sys.global_cpu_info().cpu_usage().round() as u32;
        self.ram_used_gb = self.sys.used_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
        self.ram_total_gb = (self.sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0).max(0.1);

        let up_secs = System::uptime();
        self.uptime_hrs = (up_secs / 3600) as u32;
        self.uptime_mins = ((up_secs % 3600) / 60) as u32;

        self.disks.refresh_list();
        if let Some(d) = self.disks.list().first() {
            let total = d.total_space().max(1);
            let avail = d.available_space();
            self.disk_pct = (((total.saturating_sub(avail)) as f64 / total as f64) * 100.0) as u32;
        }

        let (bat_pct, bat_chg) = crate::win_os::query_battery();
        self.bat_pct = bat_pct;
        self.bat_charging = bat_chg;
    }
}
