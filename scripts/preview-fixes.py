# Temporary deterministic corrections before formatting and Windows validation.
from pathlib import Path
import re

def replace(name, old, new):
    p=Path(name); s=p.read_text(encoding='utf-8')
    assert old in s, (name, old[:100])
    p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')

replace('crates/ui-slint/ui/path-editor.slint','self.running = false; self.applied = root.caret-revision;','self.applied = root.caret-revision;')
replace('crates/ui-slint/ui/path-editor.slint','    accessible-description: root.error;','    accessible-role: groupbox;\n    accessible-description: root.error;')
p=Path('crates/ui-slint/ui/app-window.slint');s=p.read_text(encoding='utf-8')
a=s.index('    // Toolbar autocomplete overlays');b=s.index('    // Paleta de comandos',a)
block=s[a:b];s=s[:a]+s[b:];i=s.index('    if root.hovered-tip != "": Rectangle')
s=s[:i]+block+s[i:];p.write_text(s,encoding='utf-8',newline='\n')
replace('Cargo.toml','# Metadata/preview de PDF. Versión única para core y ui-slint. OJO: `pdf-extract 0.10`\n# arrastra su propio lopdf 0.38 de forma transitiva; no se puede unificar sin cambiar\n# pdf-extract, así que esa segunda versión queda (aceptado).','# Metadata y preview de PDF: una versión de lopdf compartida; extracción acotada\n# desde el documento ya cargado, sin la antigua dependencia pdf-extract.')
replace('crates/core/src/config/mod.rs','pub const CONFIG_VERSION: u32 = 4;','pub const CONFIG_VERSION: u32 = 5;')
replace('crates/core/src/config/mod.rs','} else if loaded_version < 4 {','} else if loaded_version < 5 {')
replace('crates/core/src/config/mod.rs','    #[test]\n    fn settings_v3_recibe_reglas_3d_sin_pisar_preferencias()', '''    #[test]
    fn settings_v4_adds_image_formats_without_enabling_disabled_webp() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings { version: 4,
            preview_rules: vec![crate::preview::PreviewRule { ext: "webp".into(), enabled: false,
                view: crate::preview::ViewMode::Auto }], ..Settings::default() };
        save_settings(dir.path(), &settings);
        let loaded = load_settings(dir.path());
        assert_eq!(loaded.version, CONFIG_VERSION);
        assert!(loaded.preview_rules.iter().any(|r| r.ext == "webp" && !r.enabled));
        for ext in ["wepb", "apng", "pnm", "pbm", "pgm", "ppm", "pam", "tga"] {
            assert!(loaded.preview_rules.iter().any(|r| r.ext == ext && r.enabled), "{ext}");
        }
    }

    #[test]
    fn settings_v3_recibe_reglas_3d_sin_pisar_preferencias()''')
p=Path('crates/platform/src/preview_process.rs')
p.write_text(p.read_text(encoding='utf-8')+'''
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn closing_budget_terminates_decoder_without_an_interactive_desktop() {
        let mut command=Command::new("cmd.exe");
        command.args(["/D", "/C", "ping -n 60 127.0.0.1 >nul"]);
        configure(&mut command);
        let mut child=command.spawn().unwrap();
        let budget=Budget::attach(&child,256*1024*1024).unwrap();
        drop(budget);
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if child.try_wait().unwrap().is_some() {break;}
            if std::time::Instant::now()>=deadline {
                let _=child.kill();let _=child.wait();panic!("Job close did not terminate child");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
''',encoding='utf-8',newline='\n')
replace('crates/ui-slint/src/listing.rs','use std::sync::mpsc::Receiver;','use std::sync::mpsc::Receiver;\nuse std::cell::Cell;\nuse std::time::{Instant, Duration};')
replace('crates/ui-slint/src/listing.rs','    fresh: bool,','    fresh: bool,\n    started: Instant,\n    first_content_ms: Cell<Option<u128>>,\n    peak_poll_us: Cell<u128>,')
replace('crates/ui-slint/src/listing.rs','            fresh: true,','            fresh: true,\n            started: Instant::now(), first_content_ms: Cell::new(None), peak_poll_us: Cell::new(0),')
replace('crates/ui-slint/src/listing.rs','    /// Drena TODO lo acumulado','    /// Drena un lote acotado (hasta 1024 entradas / 3 ms de recogida) de lo acumulado')
replace('crates/ui-slint/src/listing.rs','        let mut batch = Vec::new();','        let began=Instant::now();\n        let mut batch = Vec::new();')
replace('crates/ui-slint/src/listing.rs','        while let Ok(msg) = self.rx.try_recv() {','        while batch.len()<1024 && began.elapsed()<Duration::from_millis(3) {\n            let Ok(msg)=self.rx.try_recv() else {break;};')
replace('crates/ui-slint/src/listing.rs','        (batch, done, succeeded)\n    }','''        if (!batch.is_empty() || done) && self.first_content_ms.get().is_none() {
            self.first_content_ms.set(Some(self.started.elapsed().as_millis()));
        }
        self.peak_poll_us.set(self.peak_poll_us.get().max(began.elapsed().as_micros()));
        (batch, done, succeeded)
    }
    pub fn metrics(&self)->String {
        format!("listing_metrics elapsed_ms={} first_content_ms={:?} peak_queue_poll_us={}",
            self.started.elapsed().as_millis(),self.first_content_ms.get(),self.peak_poll_us.get())
    }''')
p=Path('crates/ui-slint/src/listing.rs')
p.write_text(p.read_text(encoding='utf-8')+'''
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_queued_listing_yields_and_preserves_all_entries() {
        let (tx,rx)=std::sync::mpsc::channel();
        for n in 0..3000 {
            tx.send(ListingMsg::Entry(Entry {name:n.to_string(),path:n.to_string().into(),
                kind:naygo_core::fs_model::EntryKind::File,size:Some(0),modified:None,created:None,
                hidden:false,system:false})).unwrap();
        }
        tx.send(ListingMsg::Done).unwrap();
        let l=Listing {rx,token:CancellationToken::new(),fresh:true,started:Instant::now(),
            first_content_ms:Cell::new(None),peak_poll_us:Cell::new(0)};
        let mut total=0;let mut polls=0;
        loop {
            let (batch,done,success)=l.poll();
            assert!(batch.len()<=1024);total+=batch.len();polls+=1;
            if done {assert!(success);break;}
            assert!(polls<10000);
        }
        assert_eq!(total,3000);assert!(polls>=3);assert!(l.first_content_ms.get().is_some());
    }
}
''',encoding='utf-8',newline='\n')
replace('crates/ui-slint/src/workspace_ctrl/listing.rs','                    let (b, d, ok) = l.poll();','''                    let (b, d, ok) = l.poll();
                    if d && self.config.settings.preview_policy.diagnostics {
                        crate::logging::enqueue_metric(l.metrics());
                    }''')
p=Path('crates/ui-slint/src/logging.rs')
p.write_text(p.read_text(encoding='utf-8')+'''
/// Optional numeric diagnostics. Never perform log-file I/O from the UI thread.
/// One bounded writer for the application; saturated diagnostics are discarded.
pub fn enqueue_metric(message: String) {
    static WRITER: OnceLock<std::sync::mpsc::SyncSender<String>> = OnceLock::new();
    let tx=WRITER.get_or_init(|| {
        let (tx,rx)=std::sync::mpsc::sync_channel::<String>(16);
        std::thread::spawn(move || {for line in rx {log_line(&line);}});
        tx
    });
    let _=tx.try_send(message);
}
''',encoding='utf-8',newline='\n')
p=Path('docs/BACKLOG.md');s=p.read_text(encoding='utf-8')
s=s.replace('> Última actualización: 2026-09-07.','> Última actualización: 2026-09-09.')
s=s.replace('## Estado de partida\n','''## Estado de partida

- Revisión 2026-09-09 en rama `feat/responsive-previews-and-paths`: animaciones WebP/GIF/APNG
  con procesos cancelables y presupuestos; barra de ruta compartida con editor nativo,
  copia/favoritos; seis temas claros; configuración de íconos centralizada.
  Listado por lotes acotados y métricas locales opcionales. Ver
  [diseño y matriz de validación](PREVIEW_AND_PATHS.md).
  Estado de compilación, pruebas y distribución: consultar el workflow de esta rama;
  pendiente validación visual instalada y AltGr en el teclado del usuario.
''')
p.write_text(s,encoding='utf-8',newline='\n')
p=Path('docs/PREVIEW_AND_PATHS.md');s=p.read_text(encoding='utf-8')
s+='''\n## Configuración y navegación\n\nLa configuración migra a versión 5 para incorporar formatos nuevos en instalaciones
existentes; respeta reglas deshabilitadas y modos personalizados. Antes de probar la rama,
conservar una copia del directorio de configuración: versiones antiguas no reconocen el esquema 5.
El drenado del listado cede después de 1024 entradas o 3 ms de recogida, sin descartar
entradas. Esto no establece un límite duro para el ordenamiento o el renderizado.
Con diagnóstico activado se registran `listing_metrics` (tiempo al primer lote, tiempo
total y máximo de recogida del canal), sin rutas; la escritura usa una cola de 16
elementos y descarta métricas al saturarse, sin bloquear la UI.
'''
p.write_text(s,encoding='utf-8',newline='\n')
for p in Path('crates').rglob('*.rs'):
    s=p.read_text(encoding='utf-8')
    t=re.sub(r'\.chunks_exact_mut\((\d+)\)',r'.as_chunks_mut::<\1>().0',s)
    t=re.sub(r'\.chunks_exact\((\d+)\)',r'.as_chunks::<\1>().0.iter()',t)
    if t!=s:p.write_text(t,encoding='utf-8',newline='\n')
