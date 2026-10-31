// Naygo — experimento OLE con captura de handles en worker, sin integración en la app.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

#[cfg(not(windows))]
fn main() {
    eprintln!("Solo Windows");
}

#[cfg(windows)]
mod win {
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};
    use windows::core::{implement, w, Interface, Ref};
    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINTL, WPARAM,
    };
    use windows::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream;
    use windows::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, CoWaitForMultipleHandles, IDataObject, IStream,
        COINIT_MULTITHREADED, COWAIT_DISPATCH_CALLS, COWAIT_DISPATCH_WINDOW_MESSAGES,
        DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, OleUninitialize, RegisterDragDrop,
        ReleaseStgMedium, RevokeDragDrop, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::System::Threading::{CreateEventW, GetCurrentThreadId, SetEvent};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PeekMessageW,
        PostQuitMessage, PostThreadMessageW, RegisterClassW, SetTimer, ShowWindow,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, MSG, PM_REMOVE, SW_SHOW,
        WINDOW_EX_STYLE, WM_DESTROY, WM_QUIT, WM_TIMER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    static TICKS: AtomicU64 = AtomicU64::new(0);
    #[cfg(test)]
    static SOURCE_DELAY_ACTIVE: AtomicBool = AtomicBool::new(false);
    #[cfg(test)]
    static SOURCE_DELAY_TICKS: AtomicU64 = AtomicU64::new(0);
    const REPARSE: u32 = 0x400;
    const OPEN_REPARSE: u32 = 0x0020_0000;
    const BACKUP: u32 = 0x0200_0000;
    const READ_ATTR: u32 = 0x80;
    const LIST_DIR: u32 = 0x1;
    const SHARE_READ_WRITE: u32 = 3;
    const SHARE_ALL: u32 = 7;
    const MAX_FILES: usize = 5_000;

    struct Event(isize);
    impl Drop for Event {
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(HANDLE(self.0 as *mut _)) };
        }
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    struct Snapshot {
        files: Vec<(PathBuf, File)>,
        dirs: Vec<File>,
        dir_count: usize,
        dir_names: Vec<PathBuf>,
        root: PathBuf,
    }
    fn format() -> FORMATETC {
        FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }
    fn open(path: &Path, dir: bool) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.read(true).custom_flags(OPEN_REPARSE);
        if dir {
            options
                .access_mode(READ_ATTR | LIST_DIR)
                .share_mode(SHARE_READ_WRITE)
                .custom_flags(OPEN_REPARSE | BACKUP);
        } else {
            options.share_mode(SHARE_ALL);
        }
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if metadata.file_attributes() & REPARSE != 0 || metadata.is_dir() != dir {
            return Err(io::Error::other("reparse o tipo inesperado"));
        }
        Ok(file)
    }
    fn check_cancel(cancelled: &AtomicBool) -> io::Result<()> {
        if cancelled.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "captura cancelada",
            ))
        } else {
            Ok(())
        }
    }
    fn collect(
        path: &Path,
        relative: &Path,
        depth: usize,
        snap: &mut Snapshot,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        check_cancel(cancelled)?;
        if depth > 32 {
            return Err(io::Error::other("profundidad excedida"));
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_attributes() & REPARSE != 0 {
            return Err(io::Error::other("reparse rechazado"));
        }
        if metadata.is_dir() {
            snap.dirs.push(open(path, true)?);
            snap.dir_count += 1;
            snap.dir_names.push(relative.to_owned());
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                collect(
                    &entry.path(),
                    &relative.join(entry.file_name()),
                    depth + 1,
                    snap,
                    cancelled,
                )?;
            }
        } else if metadata.is_file() {
            if snap.files.len() >= MAX_FILES {
                return Err(io::Error::other("límite de archivos"));
            }
            snap.files.push((relative.to_owned(), open(path, false)?));
        } else {
            return Err(io::Error::other("tipo inesperado"));
        }
        Ok(())
    }
    fn capture(raw: usize, cancelled: &AtomicBool) -> Result<Snapshot, String> {
        let stream = unsafe { IStream::from_raw(raw as *mut _) };
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|e| e.to_string())?;
        let _com = Com;
        let result: windows::core::Result<IDataObject> =
            unsafe { CoGetInterfaceAndReleaseStream(&stream) };
        std::mem::forget(stream);
        let data = result.map_err(|e| e.to_string())?;
        let mut medium = unsafe { data.GetData(&format()) }.map_err(|e| e.to_string())?;
        if let Err(error) = check_cancel(cancelled) {
            unsafe { ReleaseStgMedium(&mut medium) };
            return Err(error.to_string());
        }
        if medium.tymed != TYMED_HGLOBAL.0 as u32 {
            unsafe { ReleaseStgMedium(&mut medium) };
            return Err("medio inesperado".into());
        }
        let hdrop = unsafe { HDROP(medium.u.hGlobal.0) };
        let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
        if count != 1 {
            unsafe { ReleaseStgMedium(&mut medium) };
            return Err("el experimento acepta una sola carpeta de fixture".into());
        }
        let mut roots = Vec::new();
        for index in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, index, None) } as usize;
            if len == 0 || len > 32_767 {
                unsafe { ReleaseStgMedium(&mut medium) };
                return Err("ruta inválida".into());
            }
            let mut buffer = vec![0u16; len + 1];
            let written = unsafe { DragQueryFileW(hdrop, index, Some(&mut buffer)) } as usize;
            if written != len {
                unsafe { ReleaseStgMedium(&mut medium) };
                return Err("ruta incompleta".into());
            }
            use std::os::windows::ffi::OsStringExt;
            roots.push(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..written],
            )));
        }
        unsafe { ReleaseStgMedium(&mut medium) };
        if roots.len() != 1 {
            return Err("el experimento acepta una sola carpeta de fixture".into());
        }
        let root = &roots[0];
        let temp = fs::canonicalize(std::env::temp_dir()).map_err(|e| e.to_string())?;
        let source_parent = root.parent().ok_or("sin padre")?;
        // Retener primero el padre sin seguir reparse points; después se opera desde su ruta
        // canónica. Los antecesores superiores aún requieren una solución de producción.
        let parent_guard = open(source_parent, true).map_err(|e| e.to_string())?;
        let parent = fs::canonicalize(source_parent).map_err(|e| e.to_string())?;
        if parent.parent() != Some(temp.as_path())
            || !parent
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("7zE"))
            || root.file_name().and_then(|s| s.to_str()) != Some("E2 fixture")
        {
            return Err("solo se acepta la fixture E2 en el temporal de 7-Zip".into());
        }
        let controlled_root = parent.join(root.file_name().ok_or("sin nombre")?);
        let mut snap = Snapshot {
            files: Vec::new(),
            dirs: vec![parent_guard],
            dir_count: 0,
            dir_names: Vec::new(),
            root: controlled_root.clone(),
        };
        collect(&controlled_root, Path::new(""), 0, &mut snap, cancelled)
            .map_err(|e| e.to_string())?;
        check_cancel(cancelled).map_err(|e| e.to_string())?;
        Ok(snap)
    }

    #[implement(IDropTarget, Agile = false)]
    struct Probe {
        accepted: AtomicBool,
        report: Option<mpsc::Sender<bool>>,
        callback_thread: Option<Arc<AtomicU32>>,
        baseline: PathBuf,
        expected_files: usize,
        expected_empty: PathBuf,
    }
    impl IDropTarget_Impl for Probe_Impl {
        fn DragEnter(
            &self,
            data: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            let accepted = data
                .as_ref()
                .is_some_and(|d| unsafe { d.QueryGetData(&format()).is_ok() });
            self.accepted.store(accepted, Ordering::Relaxed);
            unsafe {
                if let Some(effect) = effect.as_mut() {
                    *effect = if accepted {
                        DROPEFFECT_COPY
                    } else {
                        DROPEFFECT_NONE
                    };
                }
            }
            Ok(())
        }
        fn DragOver(
            &self,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            unsafe {
                if let Some(effect) = effect.as_mut() {
                    *effect = if self.accepted.load(Ordering::Relaxed) {
                        DROPEFFECT_COPY
                    } else {
                        DROPEFFECT_NONE
                    };
                }
            }
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            self.accepted.store(false, Ordering::Relaxed);
            Ok(())
        }
        fn Drop(
            &self,
            data: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            if let Some(observed) = &self.callback_thread {
                observed.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            }
            unsafe {
                if let Some(effect) = effect.as_mut() {
                    *effect = DROPEFFECT_NONE;
                }
            }
            if !self.accepted.swap(false, Ordering::Relaxed) {
                return Ok(());
            }
            let data = data.as_ref().ok_or_else(windows::core::Error::empty)?;
            let event = Arc::new(Event(
                unsafe { CreateEventW(None, true, false, None) }?.0 as isize,
            ));
            let stream = unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, data) }?;
            let raw = stream.into_raw() as usize;
            let worker_event = event.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_cancelled = cancelled.clone();
            let (tx, rx) = mpsc::channel();
            let started = Instant::now();
            let tick_start = TICKS.load(Ordering::Relaxed);
            std::thread::spawn(move || {
                let result = capture(raw, &worker_cancelled);
                let _ = tx.send(result);
                let _ = unsafe { SetEvent(HANDLE(worker_event.0 as *mut _)) };
            });
            let flags = (COWAIT_DISPATCH_WINDOW_MESSAGES.0 | COWAIT_DISPATCH_CALLS.0) as u32;
            let waited =
                unsafe { CoWaitForMultipleHandles(flags, 30_000, &[HANDLE(event.0 as *mut _)]) };
            match (waited, rx.try_recv()) {
                (Ok(_), Ok(Ok(snap))) => {
                    println!(
                        "captured files={} dirs={} wait_ms={:.0} native_timer_ticks={}",
                        snap.files.len(),
                        snap.dir_count,
                        started.elapsed().as_secs_f64() * 1000.0,
                        TICKS.load(Ordering::Relaxed).saturating_sub(tick_start)
                    );
                    drop(snap.dirs);
                    let report = self.report.clone();
                    let baseline = self.baseline.clone();
                    let expected_files = self.expected_files;
                    let expected_empty = self.expected_empty.clone();
                    std::thread::spawn(move || {
                        let deadline = Instant::now() + Duration::from_secs(30);
                        let temp_removed = loop {
                            match snap.root.try_exists() {
                                Ok(false) => break true,
                                Err(e) => {
                                    eprintln!(
                                        "verify_failed: no se pudo consultar el temporal: {e}"
                                    );
                                    break false;
                                }
                                Ok(true) if Instant::now() >= deadline => break false,
                                Ok(true) => std::thread::sleep(Duration::from_millis(100)),
                            }
                        };
                        let mut bytes = 0u64;
                        let count = snap.files.len();
                        let empty_present = snap.dir_names.iter().any(|p| p == &expected_empty);
                        for (relative, file) in snap.files {
                            let verify = (|| -> io::Result<u64> {
                                let mut expected = File::open(baseline.join(relative))?;
                                let len = expected.metadata()?.len();
                                if len > 1_048_576 {
                                    return Err(io::Error::other("fixture demasiado grande"));
                                }
                                let mut expected_bytes = Vec::new();
                                expected.read_to_end(&mut expected_bytes)?;
                                let mut actual_bytes = Vec::new();
                                file.take(len + 1).read_to_end(&mut actual_bytes)?;
                                if actual_bytes != expected_bytes {
                                    return Err(io::Error::other("contenido distinto"));
                                }
                                Ok(len)
                            })();
                            match verify {
                                Ok(n) => bytes += n,
                                Err(e) => {
                                    eprintln!("verify_failed: {e}");
                                    if let Some(report) = report {
                                        let _ = report.send(false);
                                    }
                                    return;
                                }
                            }
                        }
                        let matches_fixture = count == expected_files && empty_present;
                        println!("verify_after_drop files={count} bytes={bytes} temp_removed={temp_removed} empty_present={empty_present} matches_fixture={matches_fixture}");
                        if let Some(report) = report {
                            let _ = report.send(temp_removed && matches_fixture);
                        }
                    });
                    unsafe {
                        if let Some(effect) = effect.as_mut() {
                            *effect = DROPEFFECT_COPY;
                        }
                    }
                    Ok(())
                }
                (wait, result) => {
                    // GetData es una llamada del proveedor: no se puede interrumpir desde aquí.
                    // Al retornar, el worker comprueba la señal antes de recorrer el temporal.
                    cancelled.store(true, Ordering::Release);
                    eprintln!(
                        "capture_failed wait={wait:?} result={:?}",
                        result.map(|r| r.map(|_| ()))
                    );
                    Err(windows::core::Error::empty())
                }
            }
        }
    }
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_DESTROY {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        } else if msg == WM_TIMER {
            TICKS.fetch_add(1, Ordering::Relaxed);
            #[cfg(test)]
            if SOURCE_DELAY_ACTIVE.load(Ordering::SeqCst) {
                SOURCE_DELAY_TICKS.fetch_add(1, Ordering::SeqCst);
            }
            LRESULT(0)
        } else {
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
    }
    pub fn run() -> windows::core::Result<()> {
        unsafe { OleInitialize(None)? };
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let class = w!("NaygoAsyncDropProbe");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&wc) } == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!("Naygo - experimento OLE worker (solo E2 fixture)"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                520,
                300,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        let owner_tid = unsafe { GetCurrentThreadId() };
        let raw_hwnd = hwnd.0 as isize;
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let worker_tid = unsafe { GetCurrentThreadId() };
            let result = (|| -> windows::core::Result<IDropTarget> {
                unsafe { OleInitialize(None)? };
                let target: IDropTarget = Probe {
                    accepted: AtomicBool::new(false),
                    report: None,
                    callback_thread: None,
                    baseline: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out/e2-7zip-fixtures/20261030-020929-91816ffe/archive-content/E2 fixture"),
                    expected_files: 259,
                    expected_empty: PathBuf::from("empty-folder"),
                }.into();
                unsafe { RegisterDragDrop(HWND(raw_hwnd as *mut _), &target)? };
                Ok(target)
            })();
            match result {
                Ok(target) => {
                    let _ = ready_tx.send(Ok(worker_tid));
                    let mut message = MSG::default();
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                        unsafe { DispatchMessageW(&message) };
                    }
                    let _ = unsafe { RevokeDragDrop(HWND(raw_hwnd as *mut _)) };
                    drop(target);
                    unsafe { OleUninitialize() };
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error.to_string()));
                }
            }
            let _ = done_tx.send(());
        });
        let mut message = MSG::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let worker_tid = loop {
            if let Ok(result) = ready_rx.try_recv() {
                break result.map_err(|e| {
                    windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), e)
                })?;
            }
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe { DispatchMessageW(&message) };
            }
            if Instant::now() >= deadline {
                eprintln!("registration_timeout");
                std::process::exit(2);
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        unsafe {
            SetTimer(Some(hwnd), 1, 100, None);
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        println!("async_probe_ready owner_thread={owner_tid} target_thread={worker_tid}; arrastrar exclusivamente E2 fixture");
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        unsafe { PostThreadMessageW(worker_tid, WM_QUIT, WPARAM(0), LPARAM(0))? };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if done_rx.try_recv().is_ok() {
                break;
            }
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe { DispatchMessageW(&message) };
            }
            if Instant::now() >= deadline {
                eprintln!("revoke_timeout");
                std::process::exit(3);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        worker.join().expect("worker OLE falló");
        unsafe { OleUninitialize() };
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::{BOOL, HRESULT};
        use windows::Win32::Foundation::{GlobalFree, DV_E_FORMATETC, E_NOTIMPL, S_OK};
        use windows::Win32::System::Com::{
            IAdviseSink, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0,
        };
        use windows::Win32::System::Memory::{
            GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
        };
        use windows::Win32::UI::Shell::DROPFILES;

        #[implement(IDataObject, Agile = false)]
        struct SyntheticSource {
            path: PathBuf,
            delay: Duration,
            gate: Option<Arc<SourceGate>>,
        }
        struct SourceGate {
            entered: AtomicBool,
            release: AtomicBool,
        }
        impl IDataObject_Impl for SyntheticSource_Impl {
            fn GetData(&self, requested: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
                if self.QueryGetData(requested).is_err() {
                    return Err(DV_E_FORMATETC.into());
                }
                if let Some(gate) = &self.gate {
                    gate.entered.store(true, Ordering::Release);
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while !gate.release.load(Ordering::Acquire) {
                        if Instant::now() >= deadline {
                            return Err(windows::core::Error::empty());
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
                if !self.delay.is_zero() {
                    SOURCE_DELAY_ACTIVE.store(true, Ordering::SeqCst);
                    std::thread::sleep(self.delay);
                    SOURCE_DELAY_ACTIVE.store(false, Ordering::SeqCst);
                }
                let mut names: Vec<u16> = self.path.as_os_str().encode_wide().collect();
                names.extend_from_slice(&[0, 0]);
                let header = std::mem::size_of::<DROPFILES>();
                let global = unsafe { GlobalAlloc(GMEM_MOVEABLE, header + names.len() * 2) }?;
                let base = unsafe { GlobalLock(global) }.cast::<u8>();
                if base.is_null() {
                    let _ = unsafe { GlobalFree(Some(global)) };
                    return Err(windows::core::Error::from_thread());
                }
                unsafe {
                    std::ptr::write_unaligned(
                        base.cast::<DROPFILES>(),
                        DROPFILES {
                            pFiles: header as u32,
                            fWide: true.into(),
                            ..Default::default()
                        },
                    );
                    std::ptr::copy_nonoverlapping(
                        names.as_ptr(),
                        base.add(header).cast::<u16>(),
                        names.len(),
                    );
                    let _ = GlobalUnlock(global);
                }
                Ok(STGMEDIUM {
                    tymed: TYMED_HGLOBAL.0 as u32,
                    u: STGMEDIUM_0 { hGlobal: global },
                    pUnkForRelease: std::mem::ManuallyDrop::new(None),
                })
            }
            fn GetDataHere(
                &self,
                _: *const FORMATETC,
                _: *mut STGMEDIUM,
            ) -> windows::core::Result<()> {
                Err(DV_E_FORMATETC.into())
            }
            fn QueryGetData(&self, requested: *const FORMATETC) -> HRESULT {
                if unsafe { requested.as_ref() }.is_some_and(|f| {
                    f.cfFormat == CF_HDROP.0 && f.tymed & TYMED_HGLOBAL.0 as u32 != 0
                }) {
                    S_OK
                } else {
                    DV_E_FORMATETC
                }
            }
            fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
                E_NOTIMPL
            }
            fn SetData(
                &self,
                _: *const FORMATETC,
                _: *const STGMEDIUM,
                _: BOOL,
            ) -> windows::core::Result<()> {
                Err(E_NOTIMPL.into())
            }
            fn EnumFormatEtc(&self, _: u32) -> windows::core::Result<IEnumFORMATETC> {
                Err(E_NOTIMPL.into())
            }
            fn DAdvise(
                &self,
                _: *const FORMATETC,
                _: u32,
                _: Ref<IAdviseSink>,
            ) -> windows::core::Result<u32> {
                Err(E_NOTIMPL.into())
            }
            fn DUnadvise(&self, _: u32) -> windows::core::Result<()> {
                Err(E_NOTIMPL.into())
            }
            fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
                Err(E_NOTIMPL.into())
            }
        }

        fn copy_fixture(source: &Path, dest: &Path) -> io::Result<()> {
            fs::create_dir(dest)?;
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                let dst = dest.join(entry.file_name());
                if ty.is_dir() {
                    copy_fixture(&entry.path(), &dst)?;
                } else if ty.is_file() {
                    fs::copy(entry.path(), dst)?;
                } else {
                    return Err(io::Error::other("tipo inesperado en fixture"));
                }
            }
            Ok(())
        }

        #[test]
        fn explora_cancelacion_com_con_proveedor_sintetico_bloqueado() {
            use windows::Win32::System::Com::{
                CoCancelCall, CoDisableCallCancellation, CoEnableCallCancellation,
            };
            unsafe { OleInitialize(None) }.unwrap();
            let gate = Arc::new(SourceGate {
                entered: AtomicBool::new(false),
                release: AtomicBool::new(false),
            });
            let object: IDataObject = SyntheticSource {
                path: std::env::temp_dir().join("unread-synthetic-path"),
                delay: Duration::ZERO,
                gate: Some(gate.clone()),
            }
            .into();
            let stream =
                unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, &object) }
                    .unwrap();
            let raw = stream.into_raw() as usize;
            let event = Arc::new(Event(
                unsafe { CreateEventW(None, true, false, None) }.unwrap().0 as isize,
            ));
            let finished = Arc::new(AtomicBool::new(false));
            let (tid_tx, tid_rx) = mpsc::channel();
            let (result_tx, result_rx) = mpsc::channel();
            let worker_event = event.clone();
            let worker_finished = finished.clone();
            let worker = std::thread::spawn(move || {
                unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                    .ok()
                    .unwrap();
                unsafe { CoEnableCallCancellation(None) }.unwrap();
                let stream = unsafe { IStream::from_raw(raw as *mut _) };
                let data: IDataObject = unsafe { CoGetInterfaceAndReleaseStream(&stream) }.unwrap();
                std::mem::forget(stream);
                tid_tx.send(unsafe { GetCurrentThreadId() }).unwrap();
                let result = match unsafe { data.GetData(&format()) } {
                    Ok(mut medium) => {
                        unsafe { ReleaseStgMedium(&mut medium) };
                        Ok(())
                    }
                    Err(error) => Err(error.code().0),
                };
                worker_finished.store(true, Ordering::Release);
                let _ = result_tx.send(result);
                drop(data);
                unsafe { CoDisableCallCancellation(None) }.unwrap();
                unsafe { CoUninitialize() };
                let _ = unsafe { SetEvent(HANDLE(worker_event.0 as *mut _)) };
            });
            let controller = std::thread::spawn(move || {
                unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                    .ok()
                    .unwrap();
                let tid = tid_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                while !gate.entered.load(Ordering::Acquire) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let entered = gate.entered.load(Ordering::Acquire);
                let cancel_result = unsafe { CoCancelCall(tid, 0) }.map_err(|e| e.code().0);
                std::thread::sleep(Duration::from_millis(200));
                let returned_before_source = finished.load(Ordering::Acquire);
                gate.release.store(true, Ordering::Release);
                unsafe { CoUninitialize() };
                (entered, cancel_result, returned_before_source)
            });
            let flags = (COWAIT_DISPATCH_WINDOW_MESSAGES.0 | COWAIT_DISPATCH_CALLS.0) as u32;
            unsafe { CoWaitForMultipleHandles(flags, 15_000, &[HANDLE(event.0 as *mut _)]) }
                .unwrap();
            let (entered, cancellation, returned_before_source) = controller.join().unwrap();
            worker.join().unwrap();
            let result = result_rx.recv().unwrap();
            println!("com_cancel entered={entered} request={cancellation:?} returned_before_source={returned_before_source} result={result:?}");
            assert!(entered);
            drop(object);
            unsafe { OleUninitialize() };
        }

        #[test]
        fn cancelacion_durante_getdata_espera_y_no_recorrera_el_origen() {
            unsafe { OleInitialize(None) }.unwrap();
            let gate = Arc::new(SourceGate {
                entered: AtomicBool::new(false),
                release: AtomicBool::new(false),
            });
            // La ruta deliberadamente no existe: si se intentara recorrer luego de cancelar,
            // el resultado sería un error de filesystem, no el error de cancelación esperado.
            let object: IDataObject = SyntheticSource {
                path: std::env::temp_dir().join("source-that-must-not-be-enumerated"),
                delay: Duration::ZERO,
                gate: Some(gate.clone()),
            }
            .into();
            let stream =
                unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, &object) }
                    .unwrap();
            let raw = stream.into_raw() as usize;
            let event = Arc::new(Event(
                unsafe { CreateEventW(None, true, false, None) }.unwrap().0 as isize,
            ));
            let cancelled = Arc::new(AtomicBool::new(false));
            let finished = Arc::new(AtomicBool::new(false));
            let (result_tx, result_rx) = mpsc::channel();
            let worker_event = event.clone();
            let worker_cancelled = cancelled.clone();
            let worker_finished = finished.clone();
            let worker = std::thread::spawn(move || {
                let result = capture(raw, &worker_cancelled).map(|_| ());
                worker_finished.store(true, Ordering::Release);
                let _ = result_tx.send(result);
                let _ = unsafe { SetEvent(HANDLE(worker_event.0 as *mut _)) };
            });
            let controller = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !gate.entered.load(Ordering::Acquire) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let entered = gate.entered.load(Ordering::Acquire);
                cancelled.store(true, Ordering::Release);
                std::thread::sleep(Duration::from_millis(100));
                let still_waiting = !finished.load(Ordering::Acquire);
                gate.release.store(true, Ordering::Release);
                (entered, still_waiting)
            });
            let flags = (COWAIT_DISPATCH_WINDOW_MESSAGES.0 | COWAIT_DISPATCH_CALLS.0) as u32;
            unsafe { CoWaitForMultipleHandles(flags, 15_000, &[HANDLE(event.0 as *mut _)]) }
                .unwrap();
            let (entered, still_waiting) = controller.join().unwrap();
            worker.join().unwrap();
            assert!(entered, "el proveedor no entró a GetData");
            assert!(
                still_waiting,
                "GetData no debe considerarse interrumpido por la señal"
            );
            let error = result_rx.recv().unwrap().unwrap_err();
            assert!(error.contains("captura cancelada"), "{error}");
            drop(object);
            unsafe { OleUninitialize() };
        }

        #[test]
        fn recibe_objeto_ole_sintetico_y_verifica_tras_limpieza() {
            unsafe { OleInitialize(None) }.unwrap();
            let temp = tempfile::Builder::new()
                .prefix("7zE")
                .tempdir_in(std::env::temp_dir())
                .unwrap();
            let root = temp.path().join("E2 fixture");
            let baseline_temp = tempfile::tempdir().unwrap();
            let baseline = baseline_temp.path().join("baseline");
            fs::create_dir_all(baseline.join("empty-folder")).unwrap();
            fs::create_dir_all(baseline.join("nested")).unwrap();
            for index in 0..259 {
                fs::write(
                    baseline.join("nested").join(format!("file-{index:03}.txt")),
                    format!("fixture-{index}\n"),
                )
                .unwrap();
            }
            copy_fixture(&baseline, &root).unwrap();
            let object: IDataObject = SyntheticSource {
                path: root.clone(),
                delay: Duration::ZERO,
                gate: None,
            }
            .into();
            let (tx, rx) = mpsc::channel();
            let target: IDropTarget = Probe {
                accepted: AtomicBool::new(false),
                report: Some(tx),
                callback_thread: None,
                baseline,
                expected_files: 259,
                expected_empty: PathBuf::from("empty-folder"),
            }
            .into();
            let point = POINTL { x: 0, y: 0 };
            let mut effect = DROPEFFECT_COPY;
            unsafe {
                target
                    .DragEnter(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
                target
                    .Drop(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
            }
            assert_eq!(effect, DROPEFFECT_COPY);
            let resolved_parent = fs::canonicalize(temp.path()).unwrap();
            let resolved_root = fs::canonicalize(&root).unwrap();
            assert_eq!(resolved_root.parent(), Some(resolved_parent.as_path()));
            assert_eq!(resolved_root.file_name().unwrap(), "E2 fixture");
            fs::remove_dir_all(&resolved_root).unwrap();
            assert!(rx.recv_timeout(Duration::from_secs(10)).unwrap());
            drop(target);
            drop(object);
            drop(temp);
            unsafe { OleUninitialize() };
        }

        #[test]
        fn receptor_en_sta_separado_captura_ole_sintetico() {
            use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, UnregisterClassW};
            unsafe { OleInitialize(None) }.unwrap();
            let temp = tempfile::Builder::new()
                .prefix("7zE")
                .tempdir_in(std::env::temp_dir())
                .unwrap();
            let root = temp.path().join("E2 fixture");
            let baseline_temp = tempfile::tempdir().unwrap();
            let baseline = baseline_temp.path().join("baseline");
            fs::create_dir_all(baseline.join("empty-folder")).unwrap();
            fs::create_dir_all(baseline.join("nested")).unwrap();
            for index in 0..259 {
                fs::write(
                    baseline.join("nested").join(format!("file-{index:03}.txt")),
                    format!("fixture-{index}\n"),
                )
                .unwrap();
            }
            copy_fixture(&baseline, &root).unwrap();

            let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
            let class = w!("NaygoAsyncProbeCrossStaTest");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(unsafe { RegisterClassW(&wc) }, 0);
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
                    w!(""),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    20,
                    20,
                    None,
                    None,
                    Some(instance),
                    None,
                )
            }
            .unwrap();
            let owner_tid = unsafe { GetCurrentThreadId() };
            let raw_hwnd = hwnd.0 as isize;
            let observed = Arc::new(AtomicU32::new(0));
            let observed_worker = observed.clone();
            let (ready_tx, ready_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            let (report_tx, report_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                unsafe { OleInitialize(None) }.unwrap();
                let worker_tid = unsafe { GetCurrentThreadId() };
                let target: IDropTarget = Probe {
                    accepted: AtomicBool::new(false),
                    report: Some(report_tx),
                    callback_thread: Some(observed_worker),
                    baseline,
                    expected_files: 259,
                    expected_empty: PathBuf::from("empty-folder"),
                }
                .into();
                unsafe { RegisterDragDrop(HWND(raw_hwnd as *mut _), &target) }.unwrap();
                let stream =
                    unsafe { CoMarshalInterThreadInterfaceInStream(&IDropTarget::IID, &target) }
                        .unwrap();
                ready_tx
                    .send((worker_tid, stream.into_raw() as usize))
                    .unwrap();
                let mut message = MSG::default();
                while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                    unsafe { DispatchMessageW(&message) };
                }
                unsafe { RevokeDragDrop(HWND(raw_hwnd as *mut _)) }.unwrap();
                drop(target);
                unsafe { OleUninitialize() };
                done_tx.send(()).unwrap();
            });
            let mut message = MSG::default();
            let deadline = Instant::now() + Duration::from_secs(10);
            let (worker_tid, raw) = loop {
                if let Ok(result) = ready_rx.try_recv() {
                    break result;
                }
                while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                    unsafe { DispatchMessageW(&message) };
                }
                assert!(
                    Instant::now() < deadline,
                    "registro cross-STA agotó el plazo"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            SOURCE_DELAY_TICKS.store(0, Ordering::SeqCst);
            assert_ne!(unsafe { SetTimer(Some(hwnd), 1, 50, None) }, 0);
            let source_root = root.clone();
            let (source_tx, source_rx) = mpsc::channel();
            let source = std::thread::spawn(move || {
                unsafe { OleInitialize(None) }.unwrap();
                let source_tid = unsafe { GetCurrentThreadId() };
                let stream = unsafe { IStream::from_raw(raw as *mut _) };
                let proxy: windows::core::Result<IDropTarget> =
                    unsafe { CoGetInterfaceAndReleaseStream(&stream) };
                std::mem::forget(stream);
                let proxy = proxy.unwrap();
                let object: IDataObject = SyntheticSource {
                    path: source_root,
                    delay: Duration::from_millis(800),
                    gate: None,
                }
                .into();
                let point = POINTL { x: 0, y: 0 };
                let mut effect = DROPEFFECT_COPY;
                unsafe {
                    proxy
                        .DragEnter(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                        .unwrap();
                    proxy
                        .Drop(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                        .unwrap();
                }
                drop(object);
                drop(proxy);
                unsafe { OleUninitialize() };
                source_tx.send((source_tid, effect.0)).unwrap();
            });
            let deadline = Instant::now() + Duration::from_secs(45);
            let (source_tid, effect) = loop {
                if let Ok(result) = source_rx.try_recv() {
                    break result;
                }
                while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                    unsafe { DispatchMessageW(&message) };
                }
                assert!(Instant::now() < deadline, "drop de prueba agotó el plazo");
                std::thread::sleep(Duration::from_millis(5));
            };
            source.join().unwrap();
            assert_eq!(effect, DROPEFFECT_COPY.0);
            assert_ne!(owner_tid, worker_tid);
            assert_ne!(owner_tid, source_tid);
            assert_ne!(worker_tid, source_tid);
            assert_eq!(observed.load(Ordering::SeqCst), worker_tid);
            assert!(
                SOURCE_DELAY_TICKS.load(Ordering::SeqCst) > 0,
                "la ventana no procesó temporizadores durante el proveedor lento"
            );
            let resolved_parent = fs::canonicalize(temp.path()).unwrap();
            let resolved_root = fs::canonicalize(&root).unwrap();
            assert_eq!(resolved_root.parent(), Some(resolved_parent.as_path()));
            assert_eq!(resolved_root.file_name().unwrap(), "E2 fixture");
            fs::remove_dir_all(&resolved_root).unwrap();
            assert!(report_rx.recv_timeout(Duration::from_secs(30)).unwrap());
            unsafe { PostThreadMessageW(worker_tid, WM_QUIT, WPARAM(0), LPARAM(0)) }.unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if done_rx.try_recv().is_ok() {
                    break;
                }
                while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                    unsafe { DispatchMessageW(&message) };
                }
                assert!(
                    Instant::now() < deadline,
                    "revocación cross-STA agotó el plazo"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            worker.join().unwrap();
            unsafe { DestroyWindow(hwnd) }.unwrap();
            unsafe { UnregisterClassW(class, Some(instance)) }.unwrap();
            drop(temp);
            unsafe { OleUninitialize() };
        }

        #[test]
        fn captura_arbol_y_retiene_archivo_tras_borrar_origen_sintetico() {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("source");
            fs::create_dir_all(root.join("nested/empty")).unwrap();
            fs::write(root.join("nested/uno.txt"), b"prueba").unwrap();
            let mut snap = Snapshot {
                files: Vec::new(),
                dirs: Vec::new(),
                dir_count: 0,
                dir_names: Vec::new(),
                root: root.clone(),
            };
            collect(&root, Path::new(""), 0, &mut snap, &AtomicBool::new(false)).unwrap();
            assert_eq!(snap.dir_count, 3);
            assert_eq!(snap.files.len(), 1);
            assert_eq!(snap.files[0].0, PathBuf::from("nested/uno.txt"));
            let canonical_temp = fs::canonicalize(temp.path()).unwrap();
            let canonical_root = fs::canonicalize(&root).unwrap();
            assert_eq!(canonical_root.parent(), Some(canonical_temp.as_path()));
            assert_eq!(canonical_root.file_name().unwrap(), "source");
            drop(snap.dirs);
            fs::remove_dir_all(&canonical_root).unwrap();
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut snap.files[0].1, &mut data).unwrap();
            assert_eq!(data, b"prueba");
        }

        #[test]
        fn cancelacion_detiene_recorrido_y_libera_handles_al_descartar_snapshot() {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("source");
            fs::create_dir(&root).unwrap();
            fs::write(root.join("file.txt"), b"fixture").unwrap();
            let mut snap = Snapshot {
                files: vec![(
                    PathBuf::from("file.txt"),
                    open(&root.join("file.txt"), false).unwrap(),
                )],
                dirs: vec![open(&root, true).unwrap()],
                dir_count: 1,
                dir_names: vec![PathBuf::new()],
                root: root.clone(),
            };
            let cancelled = AtomicBool::new(true);
            let error = collect(&root, Path::new(""), 0, &mut snap, &cancelled).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Interrupted);
            assert_eq!(snap.files.len(), 1);
            assert_eq!(snap.dir_count, 1);
            drop(snap);
            fs::remove_dir_all(&root).unwrap();
            assert!(!root.exists());
        }
    }
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    win::run()
}
