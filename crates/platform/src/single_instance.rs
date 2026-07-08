// Naygo — instancia única del proceso (mutex con nombre + evento de "muéstrate"). Aislado en platform.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Garantiza que corra UNA sola instancia de Naygo por sesión de usuario, con un mutex Win32
//! con nombre. Si ya hay una corriendo, la secundaria le avisa "muéstrate" vía un evento con
//! nombre (y opcionalmente le deja una carpeta a abrir en un archivo de *spool* en temp) y sale.
//!
//! Decisiones de diseño:
//! - **Namespace LOCAL de sesión** (sin prefijo `Global\`): la instancia única es POR SESIÓN de
//!   usuario. En un equipo multiusuario, cada usuario tiene su propio Naygo — es lo correcto.
//! - **Tolerancia total a fallos**: [`acquire`] NUNCA bloquea ni aborta el arranque. Ante
//!   cualquier error Win32 inesperado devuelve `Primary` (mejor dos instancias que ninguna).
//! - **Evento auto-reset**: un `SetEvent` despierta exactamente UN `WaitForSingleObject` y el
//!   evento se re-arma solo; no hace falta `ResetEvent` manual ni hay riesgo de re-despertares.
//! - **Sin `Drop` crítico en [`Guard`]**: los handles del mutex y del evento deben vivir toda la
//!   vida del proceso (soltarlos antes dejaría entrar una segunda instancia). Windows los libera
//!   automáticamente al morir el proceso, así que no hay nada que limpiar a mano.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::dir_watch::Waker;

/// Nombre del archivo de spool (en `%TEMP%`) donde la secundaria deja la carpeta pedida.
const SPOOL_FILE: &str = "naygo-open-request.txt";

/// Resultado de intentar ser la instancia única.
pub enum Instance {
    /// Esta es la primera instancia: sigue arrancando normal. El guard mantiene vivos el mutex
    /// y el evento; debe vivir toda la vida del proceso.
    Primary(Guard),
    /// Ya hay otra instancia corriendo: el llamador debe avisarle (`notify_running`) y salir.
    AlreadyRunning,
}

/// Retiene los handles Win32 del mutex y del evento mientras el proceso vive.
///
/// Los handles se guardan como `isize` (y no como `HANDLE`) porque `HANDLE` envuelve un puntero
/// crudo y no es `Send`; `isize` sí lo es, y así el guard puede cruzar hilos y el handle del
/// evento puede pasarse al hilo vigilante de [`Guard::watch`]. Un valor `0` marca un guard
/// inerte (sin mutex o sin evento), producto de un fallo Win32 tolerado.
///
/// A propósito NO implementa `Drop`: los handles viven toda la vida del proceso y Windows los
/// libera al terminar; cerrarlos antes de tiempo abriría la puerta a una segunda instancia.
pub struct Guard {
    /// Handle del mutex `NaygoSingleInstance` (0 = inerte). Solo se retiene, nunca se usa.
    _mutex: isize,
    /// Handle del evento `NaygoSingleInstanceShow` (0 = inerte: `watch` no hará nada).
    #[cfg_attr(not(windows), allow(dead_code))]
    event: isize,
}

impl Guard {
    /// Guard inerte: sin mutex ni evento. Se usa cuando Win32 falló (o en no-Windows) para que
    /// el arranque continúe igual — la app funciona, solo sin garantía de instancia única.
    fn inert() -> Self {
        Guard {
            _mutex: 0,
            event: 0,
        }
    }

    /// Arranca el hilo vigilante: espera el evento "muéstrate" con
    /// `WaitForSingleObject(INFINITE)` en loop; en cada disparo marca `requested` en `true` y
    /// llama `waker()` para despertar el loop de UI (que normalmente está dormido).
    ///
    /// El hilo es *detached*: muere con el proceso, no hay nada que joinear. Esperar con
    /// `INFINITE` no consume CPU (el hilo duerme en el kernel hasta el `SetEvent`).
    /// En no-Windows es un no-op.
    pub fn watch(&self, requested: Arc<AtomicBool>, waker: Waker) {
        #[cfg(windows)]
        {
            if self.event == 0 {
                // Guard inerte (el evento no se pudo crear): no hay nada que vigilar.
                return;
            }
            // El handle cruza al hilo como isize: HANDLE envuelve un puntero y no es Send.
            let event = self.event;
            let builder = std::thread::Builder::new().name("naygo-single-instance".into());
            // Si el spawn falla (recursos agotados), se tolera: la app sigue sin el "muéstrate".
            let _ = builder.spawn(move || {
                use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
                use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};

                let event = HANDLE(event as *mut core::ffi::c_void);
                loop {
                    let r = unsafe { WaitForSingleObject(event, INFINITE) };
                    if r != WAIT_OBJECT_0 {
                        // WAIT_FAILED (handle inválido, etc.): salir del loop para no quedar
                        // girando en un busy-loop de errores. El hilo termina en silencio.
                        break;
                    }
                    requested.store(true, std::sync::atomic::Ordering::SeqCst);
                    waker();
                }
            });
        }
        #[cfg(not(windows))]
        {
            // Stub no-Windows: no hay evento que vigilar.
            let _ = (requested, waker);
        }
    }
}

/// Intenta ser la instancia única. NUNCA bloquea ni falla el arranque: ante cualquier error
/// Win32 inesperado, devuelve `Primary` (mejor dos instancias que ninguna).
///
/// Mecanismo: crea el mutex con nombre `NaygoSingleInstance`. Si el SO reporta
/// `ERROR_ALREADY_EXISTS`, otro proceso ya lo tiene → [`Instance::AlreadyRunning`]. Si somos
/// los primeros, crea además el evento auto-reset `NaygoSingleInstanceShow` con el que las
/// secundarias nos pedirán mostrarnos.
#[cfg(windows)]
pub fn acquire() -> Instance {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{CreateEventW, CreateMutexW};

    unsafe {
        // Sin prefijo "Global\": namespace local de sesión → instancia única POR SESIÓN de
        // usuario (correcto para multiusuario). `false` = no tomamos posesión del mutex, solo
        // lo usamos como marcador de existencia.
        let mutex = match CreateMutexW(None, false, w!("NaygoSingleInstance")) {
            Ok(h) => h,
            Err(e) => {
                // Error inesperado del SO. Se usa eprintln porque `platform` no conoce el
                // logger de la capa UI (aislamiento de capas). Nunca abortar el arranque.
                eprintln!("naygo: CreateMutexW falló ({e}); se arranca sin instancia única");
                return Instance::Primary(Guard::inert());
            }
        };
        // GetLastError JUSTO tras el CreateMutexW exitoso (nada en medio que lo pise):
        // ERROR_ALREADY_EXISTS = el mutex ya existía → hay otra instancia viva.
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // Cerrar el handle recibido: la secundaria no debe retener el mutex ajeno
            // (mantendría "viva" la marca aunque la primaria muriera).
            let _ = CloseHandle(mutex);
            return Instance::AlreadyRunning;
        }

        // Somos la primaria: descartar un spool STALE de una sesión anterior. Si la primaria
        // de esa sesión murió (crash/kill) después de que una secundaria escribió el spool sin
        // llegar a consumirlo, quedaría un pedido de carpeta viejo en %TEMP% — y el PRÓXIMO
        // "muéstrate" (relanzar el exe sin carpeta) abriría un panel con una carpeta que nadie
        // pidió en esta sesión. Un pedido de carpeta solo es válido dentro de la sesión de la
        // primaria que lo recibe.
        let _ = std::fs::remove_file(spool_path());

        // Crear el evento "muéstrate". Auto-reset (`bmanualreset = false`):
        // un SetEvent despierta UN WaitForSingleObject y el evento se re-arma solo.
        let event = match CreateEventW(None, false, false, w!("NaygoSingleInstanceShow")) {
            Ok(h) => h.0 as isize,
            Err(e) => {
                // Sin evento seguimos siendo instancia única; solo se pierde el "muéstrate".
                eprintln!("naygo: CreateEventW falló ({e}); sin aviso de 'muéstrate'");
                0
            }
        };

        Instance::Primary(Guard {
            _mutex: mutex.0 as isize,
            event,
        })
    }
}

/// Stub no-Windows: no hay mutex con nombre; siempre somos la primaria (guard inerte).
#[cfg(not(windows))]
pub fn acquire() -> Instance {
    Instance::Primary(Guard::inert())
}

/// Desde la instancia SECUNDARIA: deja la carpeta pedida (si hay) en el spool y dispara el
/// evento para que la primaria se muestre. Tolerante: si algo falla (la primaria murió justo
/// ahora, el evento no existe, temp no escribible), no hace nada — la secundaria sale igual.
#[cfg(windows)]
pub fn notify_running(folder: Option<&Path>) {
    use windows::core::w;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};

    // Primero el spool y DESPUÉS el evento: cuando la primaria despierte, la carpeta ya debe
    // estar escrita. Si la escritura falla, el SetEvent igual vale (mostrarse ya es útil).
    if let Some(f) = folder {
        let _ = std::fs::write(spool_path(), f.to_string_lossy().as_bytes());
    }
    unsafe {
        // Solo pedimos EVENT_MODIFY_STATE (lo mínimo para SetEvent).
        if let Ok(event) = OpenEventW(EVENT_MODIFY_STATE, false, w!("NaygoSingleInstanceShow")) {
            let _ = SetEvent(event);
            let _ = CloseHandle(event);
        }
    }
}

/// Stub no-Windows: no hay a quién avisar.
#[cfg(not(windows))]
pub fn notify_running(_folder: Option<&Path>) {}

/// Ruta FIJA del archivo de spool en temp. Fija a propósito: primaria y secundaria son procesos
/// distintos y deben coincidir en la ruta sin coordinarse.
fn spool_path() -> PathBuf {
    std::env::temp_dir().join(SPOOL_FILE)
}

/// Lee Y BORRA el archivo de spool con la carpeta pedida por una secundaria. `None` si no hay,
/// no parsea, o la ruta no existe como carpeta (defensa: el spool podría ser viejo o basura).
/// Cross-platform a propósito (el spool es solo filesystem), para poder testearlo en cualquier OS.
pub fn take_open_request() -> Option<PathBuf> {
    take_open_request_at(&spool_path())
}

/// Implementación con ruta parametrizable, para que los tests no choquen con la ruta fija real.
fn take_open_request_at(spool: &Path) -> Option<PathBuf> {
    let contenido = std::fs::read_to_string(spool).ok()?;
    // Borrar SIEMPRE tras leer (consumo destructivo). Si el borrado falla se ignora: el peor
    // caso es re-abrir la misma carpeta en el próximo arranque, inofensivo.
    let _ = std::fs::remove_file(spool);
    let ruta = PathBuf::from(contenido.trim());
    // Defensa contra spool viejo/corrupto: solo devolver rutas que existen como carpeta HOY.
    if ruta.is_dir() {
        Some(ruta)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spool_roundtrip_devuelve_carpeta_y_borra_el_archivo() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("spool.txt");
        // La carpeta pedida debe existir: usamos el propio tempdir.
        std::fs::write(&spool, dir.path().to_string_lossy().as_bytes()).unwrap();

        let leido = take_open_request_at(&spool);
        assert_eq!(leido, Some(dir.path().to_path_buf()));
        // Consumo destructivo: el spool ya no debe existir.
        assert!(!spool.exists(), "el spool debe borrarse tras leerse");
    }

    #[test]
    fn spool_con_salto_de_linea_final_se_recorta() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("spool.txt");
        let contenido = format!("{}\r\n", dir.path().display());
        std::fs::write(&spool, contenido).unwrap();

        assert_eq!(take_open_request_at(&spool), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn sin_archivo_de_spool_es_none() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("no-existe.txt");
        assert_eq!(take_open_request_at(&spool), None);
    }

    #[test]
    fn spool_con_ruta_inexistente_es_none_y_se_consume() {
        let dir = tempfile::tempdir().unwrap();
        let spool = dir.path().join("spool.txt");
        std::fs::write(&spool, "Z:/carpeta/que/no/existe/naygo").unwrap();

        assert_eq!(take_open_request_at(&spool), None);
        // Aunque la ruta sea basura, el spool se consume igual (no debe quedar pegado).
        assert!(
            !spool.exists(),
            "el spool debe borrarse aunque la ruta sea inválida"
        );
    }
}
