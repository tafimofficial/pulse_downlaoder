use std::process::Command;

pub fn set_autostart(enable: bool) {
    if let Ok(current_exe) = std::env::current_exe() {
        let exe_str = current_exe.to_string_lossy().to_string();
        let cmd_val = format!("\"{}\" --minimized", exe_str);

        if enable {
            let mut cmd = Command::new("reg");
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            let _ = cmd
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "PulseDL",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &cmd_val,
                    "/f",
                ])
                .output();
        } else {
            let mut cmd = Command::new("reg");
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            let _ = cmd
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "PulseDL",
                    "/f",
                ])
                .output();
        }
    }
}
