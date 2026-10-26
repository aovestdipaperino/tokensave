//! Windows console identity. Never write terminal escapes to MCP stdout.

#[cfg(windows)]
pub struct ConsoleIdentity(Vec<u16>);

#[cfg(windows)]
impl ConsoleIdentity {
    pub fn enter() -> Option<Self> {
        use std::io::IsTerminal;
        use windows_sys::Win32::{System::Console, UI::Shell};

        if !tokensave::identity::is_beta() {
            return None;
        }
        let app_id: Vec<u16> = tokensave::identity::APPLICATION_ID
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: the UTF-16 application ID is NUL terminated and lives through the call.
        unsafe {
            Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
        }
        if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() {
            return None;
        }
        let mut previous = vec![0_u16; 32768];
        let title: Vec<u16> = tokensave::identity::PRODUCT_NAME
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: all buffers are valid UTF-16 storage; input strings are NUL terminated.
        unsafe {
            let length = Console::GetConsoleTitleW(previous.as_mut_ptr(), previous.len() as u32);
            previous.truncate(length as usize + 1);
            Console::SetConsoleTitleW(title.as_ptr());
        }
        Some(Self(previous))
    }
}

#[cfg(windows)]
impl Drop for ConsoleIdentity {
    fn drop(&mut self) {
        // SAFETY: GetConsoleTitleW populated this owned, NUL-terminated buffer.
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleTitleW(self.0.as_ptr());
        }
    }
}
