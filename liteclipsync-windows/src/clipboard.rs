use image::ImageEncoder;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

static FILE_CACHE: Mutex<Option<(PathBuf, std::time::SystemTime, ClipboardContent)>> = Mutex::new(None);

const CF_HDROP: u32 = 15;

#[cfg(windows)]
fn get_clipboard_files() -> Vec<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    let mut files = Vec::new();
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return files;
        }
        let handle = GetClipboardData(CF_HDROP);
        if handle.is_null() {
            CloseClipboard();
            return files;
        }
        // Query file count (FALSE returns 0, passing 0xFFFFFFFF as ifile gives count)
        let count = DragQueryFileW(handle as _, 0xFFFFFFFF, std::ptr::null_mut(), 0);
        for i in 0..count {
            let len = DragQueryFileW(handle as _, i, std::ptr::null_mut(), 0);
            if len == 0 { continue; }
            let mut buf = vec![0u16; len as usize];
            DragQueryFileW(handle as _, i, buf.as_mut_ptr(), len + 1);
            buf.truncate(buf.iter().position(|&c| c == 0).unwrap_or(buf.len()));
            files.push(PathBuf::from(OsString::from_wide(&buf)));
        }
        CloseClipboard();
    }
    files
}

#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
    Image {
        png_bytes: Vec<u8>,
    },
    File {
        name: String,
        bytes: Vec<u8>,
    },
}

impl ClipboardContent {
    pub fn content_type(&self) -> &str {
        match self {
            Self::Text(_) => "Text",
            Self::Image { .. } => "Image",
            Self::File { .. } => "File",
        }
    }

    pub fn compute_hash(&self) -> String {
        match self {
            Self::Text(t) => {
                let mut h = Sha256::new();
                h.update(t.as_bytes());
                hex::encode(h.finalize())
            }
            Self::Image { png_bytes, .. } => {
                let mut h = Sha256::new();
                h.update(png_bytes);
                hex::encode(h.finalize())
            }
            Self::File { bytes, .. } => {
                let mut h = Sha256::new();
                h.update(bytes);
                hex::encode(h.finalize())
            }
        }
    }

    pub fn size(&self) -> i64 {
        match self {
            Self::Text(t) => t.len() as i64,
            Self::Image { png_bytes, .. } => png_bytes.len() as i64,
            Self::File { bytes, .. } => bytes.len() as i64,
        }
    }
}

pub struct ClipboardMonitor {
    last_hash: Option<String>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self { last_hash: None }
    }

    pub fn read_and_check(&mut self) -> Option<ClipboardContent> {
        let content = read_clipboard();
        match content {
            Some(c) => {
                let hash = c.compute_hash();
                if self.last_hash.as_deref() == Some(&hash) {
                    return None;
                }
                self.last_hash = Some(hash);
                Some(c)
            }
            None => None,
        }
    }

    pub fn set_last_hash(&mut self, hash: &str) {
        self.last_hash = Some(hash.to_string());
    }
}

pub(crate) fn read_clipboard() -> Option<ClipboardContent> {
    // ── Phase 1: 检测文件路径（最优先，避免先打开剪贴板触发格式转换）──
    #[cfg(windows)]
    {
        let files = get_clipboard_files();
        if !files.is_empty() {
            for path in &files {
                if !path.exists() { continue; }
                // 检查文件 mtime 缓存，避免重复读盘
                let mtime = path.metadata().ok().and_then(|m| m.modified().ok());
                if let Some((ref cached_path, ref cached_mtime, ref cached_content)) =
                    *FILE_CACHE.lock().unwrap_or_else(|e| e.into_inner())
                {
                    if cached_path == path && Some(cached_mtime) == mtime.as_ref() {
                        return Some(cached_content.clone());
                    }
                }
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp") {
                    debug!("clipboard file image: {} ({})", path.display(), ext);
                    match std::fs::read(path) {
                        Ok(bytes) => {
                            let content = if ext == "png" {
                                ClipboardContent::Image { png_bytes: bytes }
                            } else if let Ok(img) = image::load_from_memory(&bytes) {
                                let rgba = img.to_rgba8();
                                let (w, h) = rgba.dimensions();
                                let mut buf = std::io::Cursor::new(Vec::new());
                                let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                                encoder.write_image(&rgba, w, h, image::ExtendedColorType::Rgba8).ok();
                                let png = buf.into_inner();
                                debug!("  decoded {} {}x{} re-encoded {} bytes", ext, w, h, png.len());
                                ClipboardContent::Image { png_bytes: png }
                            } else {
                                warn!("  failed to decode {} as image", path.display());
                                continue;
                            };
                            if let Some(ts) = mtime {
                                *FILE_CACHE.lock().unwrap() = Some((path.clone(), ts, content.clone()));
                            }
                            return Some(content);
                        }
                        Err(e) => warn!("  failed to read file {}: {e}", path.display()),
                    }
                } else {
                    debug!("clipboard file: {} (generic)", path.display());
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unnamed")
                        .to_string();
                    match std::fs::read(path) {
                        Ok(bytes) => {
                            let content = ClipboardContent::File { name, bytes };
                            if let Some(ts) = mtime {
                                *FILE_CACHE.lock().unwrap() = Some((path.clone(), ts, content.clone()));
                            }
                            return Some(content);
                        }
                        Err(e) => warn!("  failed to read file {}: {e}", path.display()),
                    }
                }
            }
        }
    }

    // ── Phase 2: 检测文本 + 位图（arboard 单次打开）──
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(text) = cb.get_text() {
            return Some(ClipboardContent::Text(text));
        }
        if let Ok(img) = cb.get_image() {
            let (w, h) = (img.width, img.height);
            let png_bytes = encode_png(&img.bytes, w, h);
            info!("clipboard bitmap image: {}x{} encoded {} bytes", w, h, png_bytes.len());
            if png_bytes.is_empty() {
                warn!("PNG encoding produced empty data, skipping");
                return None;
            }
            return Some(ClipboardContent::Image { png_bytes });
        }
    }

    None
}

pub fn encode_png(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    if let Err(e) = encoder.write_image(rgba, width as u32, height as u32, image::ExtendedColorType::Rgba8) {
        warn!("PNG encode failed: {e}");
    }
    buf.into_inner()
}

pub fn set_clipboard(content: &ClipboardContent) {
    let mut cb = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open clipboard: {e}");
            return;
        }
    };

    match content {
        ClipboardContent::Text(t) => {
            if let Err(e) = cb.set_text(t.clone()) {
                warn!("failed to set clipboard text: {e}");
            }
        }
        ClipboardContent::Image { png_bytes, .. } => {
            match decode_png(png_bytes) {
                Some((rgba, w, h)) => {
                    let img = arboard::ImageData {
                        width: w,
                        height: h,
                        bytes: rgba.into(),
                    };
                    if let Err(e) = cb.set_image(img) {
                        warn!("failed to set clipboard image: {e}");
                    }
                }
                None => warn!("failed to decode received PNG"),
            }
        }
        ClipboardContent::File { name, bytes } => {
            drop(cb);
            let dir = std::env::temp_dir().join("liteclipsync");
            let path = dir.join(name);
            std::fs::create_dir_all(&dir).ok();
            if let Err(e) = std::fs::write(&path, bytes) {
                warn!("failed to save received file: {e}");
                return;
            }
            info!("saved received file: {} ({} bytes)", path.display(), path.metadata().map(|m| m.len()).unwrap_or(0));

            // 设置 CF_HDROP 剪贴板，让用户可以像正常复制文件一样粘贴
            #[cfg(windows)]
            unsafe {
                use std::ffi::OsStr;
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::System::DataExchange::{CloseClipboard, OpenClipboard, SetClipboardData};
                use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};
                const GHND: u32 = 0x0042;
                const CF_HDROP: u32 = 15;

                let path_wide: Vec<u16> = OsStr::new(path.as_os_str()).encode_wide().collect();
                // DROPFILES: p_files(4) + pt(8) + fNC(4) + fWide(4) = 20 bytes
                let dropfiles_size = 20u32;
                let path_bytes = (path_wide.len() * 2) as u32; // wide chars in bytes
                let total_size = (dropfiles_size + path_bytes + 4) as usize; // header + data + 2 nulls

                let h = GlobalAlloc(GHND, total_size);
                if h.is_null() {
                    warn!("  GlobalAlloc failed");
                    return;
                }
                let ptr = GlobalLock(h) as *mut u8;
                if ptr.is_null() {
                    warn!("  GlobalLock failed");
                    return;
                }
                // Write DROPFILES header
                *(ptr as *mut u32) = dropfiles_size; // p_files
                *(ptr.add(4) as *mut i32) = 0; // pt.x
                *(ptr.add(8) as *mut i32) = 0; // pt.y
                *(ptr.add(12) as *mut i32) = 0; // fNC
                *(ptr.add(16) as *mut i32) = 1; // fWide (Unicode)
                // Write path string + double null termination
                let dst = ptr.add(dropfiles_size as usize) as *mut u16;
                for (i, &ch) in path_wide.iter().enumerate() {
                    *dst.add(i) = ch;
                }
                *dst.add(path_wide.len()) = 0;     // null terminator
                *dst.add(path_wide.len() + 1) = 0; // extra null (end of list)
                GlobalUnlock(h);

                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    SetClipboardData(CF_HDROP, h);
                    CloseClipboard();
                }
            }
        }
    }
}

fn decode_png(data: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::load_from_memory(data).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w as usize, h as usize))
}

#[cfg(windows)]
pub fn simulate_copy() {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x11,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x43,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x43,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x11,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        SendInput(4, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(windows)]
pub fn simulate_paste() {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x11,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x56,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x56,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0x11,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        SendInput(4, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

